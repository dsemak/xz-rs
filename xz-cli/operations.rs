//! Compression and decompression operations for XZ CLI.

use std::fs::File;
use std::io;
use std::path::Path;

use xz_core::{
    config::EncodeFormat,
    file_info,
    options::lzma1::Lzma1Options,
    options::{
        BcjOptions, Compression, CompressionOptions, DecompressionOptions, DeltaOptions,
        FilterConfig, FilterOptions, FilterType, Flags, LzmaOptions,
    },
    pipeline::{compress, decompress},
    ratio, Error as CoreError, UnknownInputPolicy,
};

use crate::config::CliConfig;
use crate::error::{DiagnosticCause, Error, IoErrorNoCode, Result, Warning};
use crate::format::list::{self, ListOutputContext, ListSummary};
use crate::lzma1::parse_lzma1_options;

/// Resolve the output container format for compression.
fn resolve_encode_format(config: &CliConfig) -> EncodeFormat {
    if config.format == xz_core::config::DecodeMode::Lzma {
        return EncodeFormat::Lzma;
    }
    if config.format == xz_core::config::DecodeMode::Raw {
        return EncodeFormat::Raw;
    }
    EncodeFormat::Xz
}

/// Returns a human-readable error message corresponding to a `CoreError`.
fn xz_message_from_core_error(err: &CoreError) -> String {
    match err {
        CoreError::Backend(backend) => backend.xz_message().to_string(),
        CoreError::InvalidOption(message) => message.clone(),
        _ => err.to_string(),
    }
}

/// Compute the effective compression level, applying `--extreme` when requested.
fn resolve_compression_level(config: &CliConfig) -> Result<Compression> {
    // Extreme is a modifier that applies to the selected preset level, not a separate level.
    match (config.level, config.extreme) {
        (Some(level), true) => {
            let level_u8 = u8::try_from(level)
                .map_err(|_| DiagnosticCause::from(Error::InvalidCompressionLevel { level }))?;
            Ok(Compression::Extreme(level_u8))
        }
        (Some(level), false) => Compression::try_from(level)
            .map_err(|_| DiagnosticCause::from(Error::InvalidCompressionLevel { level })),
        (None, true) => {
            let preset = Compression::default().to_preset();
            // Invariant: `Compression::to_preset()` returns a valid xz preset level (`0..=9`).
            let preset_u8 = u8::try_from(preset).map_err(|_| {
                DiagnosticCause::from(Error::InvalidOption {
                    message: format!(
                        "internal error: default compression preset must fit into u8 (got {preset})"
                    ),
                })
            })?;
            Ok(Compression::Extreme(preset_u8))
        }
        (None, false) => Ok(Compression::default()),
    }
}

fn build_lzma1_options(raw_lzma1: &str, compression_level: Compression) -> Result<Lzma1Options> {
    let parsed = parse_lzma1_options(raw_lzma1).map_err(DiagnosticCause::from)?;
    let base_preset = parsed.preset.unwrap_or(compression_level);
    let mut lzma1 = Lzma1Options::from_preset(base_preset).map_err(|e| {
        DiagnosticCause::from(Error::InvalidOption {
            message: format!("unable to apply --lzma1 preset: {e}"),
        })
    })?;

    if let Some(dict) = parsed.dict_size {
        lzma1 = lzma1.with_dict_size(dict);
    }
    if let Some(lc) = parsed.lc {
        lzma1 = lzma1.with_lc(lc);
    }
    if let Some(lp) = parsed.lp {
        lzma1 = lzma1.with_lp(lp);
    }
    if let Some(pb) = parsed.pb {
        lzma1 = lzma1.with_pb(pb);
    }
    if let Some(mode) = parsed.mode {
        lzma1 = lzma1.with_mode(mode);
    }
    if let Some(nice) = parsed.nice_len {
        lzma1 = lzma1.with_nice_len(nice);
    }
    if let Some(mf) = parsed.mf {
        lzma1 = lzma1.with_match_finder(mf);
    }
    if let Some(depth) = parsed.depth {
        lzma1 = lzma1.with_depth(depth);
    }

    Ok(lzma1)
}

fn parse_bcj_options(
    filter_name: &str,
    raw_options: Option<&str>,
) -> Result<Option<FilterOptions>> {
    let Some(raw_options) = raw_options else {
        return Ok(None);
    };
    if raw_options.is_empty() {
        return Ok(Some(FilterOptions::Bcj(BcjOptions::default())));
    }

    let mut start_offset = None;
    for part in raw_options.split(',') {
        let (key, value) = part.split_once('=').ok_or_else(|| {
            DiagnosticCause::from(Error::InvalidOption {
                message: format!("invalid {filter_name} filter option: {part}"),
            })
        })?;

        match key {
            "start" => {
                let parsed = value.parse::<u32>().map_err(|_| {
                    DiagnosticCause::from(Error::InvalidOption {
                        message: format!("invalid {filter_name} start offset: {value}"),
                    })
                })?;
                start_offset = Some(parsed);
            }
            _ => {
                return Err(DiagnosticCause::from(Error::InvalidOption {
                    message: format!("unsupported {filter_name} filter option: {key}"),
                }));
            }
        }
    }

    Ok(Some(FilterOptions::Bcj(BcjOptions {
        start_offset: start_offset.unwrap_or(0),
    })))
}

fn parse_delta_options(raw_options: Option<&str>) -> Result<Option<FilterOptions>> {
    let Some(raw_options) = raw_options else {
        return Ok(Some(FilterOptions::Delta(DeltaOptions::default())));
    };
    if raw_options.is_empty() {
        return Ok(Some(FilterOptions::Delta(DeltaOptions::default())));
    }

    let mut distance = None;
    for part in raw_options.split(',') {
        let (key, value) = part.split_once('=').ok_or_else(|| {
            DiagnosticCause::from(Error::InvalidOption {
                message: format!("invalid delta filter option: {part}"),
            })
        })?;

        match key {
            "dist" => {
                let parsed = value.parse::<u32>().map_err(|_| {
                    DiagnosticCause::from(Error::InvalidOption {
                        message: format!("invalid delta distance: {value}"),
                    })
                })?;
                if !(1..=256).contains(&parsed) {
                    return Err(DiagnosticCause::from(Error::InvalidOption {
                        message: format!("delta distance out of range: {parsed}"),
                    }));
                }
                distance = Some(parsed);
            }
            _ => {
                return Err(DiagnosticCause::from(Error::InvalidOption {
                    message: format!("unsupported delta filter option: {key}"),
                }));
            }
        }
    }

    Ok(Some(FilterOptions::Delta(DeltaOptions {
        distance: distance.unwrap_or(1),
    })))
}

fn parse_filters_chain(
    raw_filters: &str,
    compression_level: Compression,
) -> Result<Vec<FilterConfig>> {
    let mut filters = Vec::new();

    for raw_filter in raw_filters.split_whitespace() {
        let (name, raw_options) = match raw_filter.split_once(':') {
            Some((name, raw_options)) => (name, Some(raw_options)),
            None => (raw_filter, None),
        };

        let filter = match name {
            "lzma2" => {
                let lzma1 =
                    build_lzma1_options(raw_options.unwrap_or_default(), compression_level)?;
                FilterConfig {
                    filter_type: FilterType::Lzma2,
                    options: Some(FilterOptions::Lzma(LzmaOptions::from(&lzma1))),
                }
            }
            "x86" => FilterConfig {
                filter_type: FilterType::X86,
                options: parse_bcj_options("x86", raw_options)?,
            },
            "powerpc" => FilterConfig {
                filter_type: FilterType::PowerPc,
                options: parse_bcj_options("powerpc", raw_options)?,
            },
            "ia64" => FilterConfig {
                filter_type: FilterType::Ia64,
                options: parse_bcj_options("ia64", raw_options)?,
            },
            "arm" => FilterConfig {
                filter_type: FilterType::Arm,
                options: parse_bcj_options("arm", raw_options)?,
            },
            "armthumb" => FilterConfig {
                filter_type: FilterType::ArmThumb,
                options: parse_bcj_options("armthumb", raw_options)?,
            },
            "arm64" => FilterConfig {
                filter_type: FilterType::Arm64,
                options: parse_bcj_options("arm64", raw_options)?,
            },
            "sparc" => FilterConfig {
                filter_type: FilterType::Sparc,
                options: parse_bcj_options("sparc", raw_options)?,
            },
            "riscv" => FilterConfig {
                filter_type: FilterType::RiscV,
                options: parse_bcj_options("riscv", raw_options)?,
            },
            "delta" => FilterConfig {
                filter_type: FilterType::Delta,
                options: parse_delta_options(raw_options)?,
            },
            _ => {
                return Err(DiagnosticCause::from(Error::InvalidOption {
                    message: format!("unsupported filter in --filters: {name}"),
                }));
            }
        };

        filters.push(filter);
    }

    if filters.is_empty() {
        return Err(DiagnosticCause::from(Error::InvalidOption {
            message: "--filters requires at least one filter".into(),
        }));
    }

    Ok(filters)
}

/// Apply `--lzma1` overrides to compression options when encoding `.lzma` or raw streams.
fn apply_lzma1_overrides(
    mut options: CompressionOptions,
    config: &CliConfig,
    encode_format: EncodeFormat,
    compression_level: Compression,
) -> Result<CompressionOptions> {
    let Some(raw_lzma1) = config.lzma1.as_deref() else {
        return Ok(options);
    };

    if !matches!(encode_format, EncodeFormat::Lzma | EncodeFormat::Raw) {
        return Err(DiagnosticCause::from(Error::InvalidOption {
            message: "--lzma1 is only supported with --format=lzma or --format=raw".into(),
        }));
    }

    let lzma1 = build_lzma1_options(raw_lzma1, compression_level)?;
    options = options.with_lzma1_options(Some(lzma1));
    Ok(options)
}

/// Apply `--lzma2` overrides to `.xz` compression options.
fn apply_lzma2_overrides(
    mut options: CompressionOptions,
    config: &CliConfig,
    encode_format: EncodeFormat,
    compression_level: Compression,
) -> Result<CompressionOptions> {
    let Some(raw_lzma2) = config.lzma2.as_deref() else {
        return Ok(options);
    };

    if encode_format != EncodeFormat::Xz {
        return Err(DiagnosticCause::from(Error::InvalidOption {
            message: "--lzma2 is only supported with .xz output".into(),
        }));
    }

    let lzma1 = build_lzma1_options(raw_lzma2, compression_level)?;
    let lzma2 = LzmaOptions::from(&lzma1);
    let filters = vec![FilterConfig {
        filter_type: FilterType::Lzma2,
        options: Some(FilterOptions::Lzma(lzma2)),
    }];

    options = options.with_filters(filters);
    Ok(options)
}

/// Apply `--filters` explicit filter-chain overrides to `.xz` compression options.
fn apply_filters_override(
    mut options: CompressionOptions,
    config: &CliConfig,
    encode_format: EncodeFormat,
    compression_level: Compression,
) -> Result<CompressionOptions> {
    let Some(raw_filters) = config.filters.as_deref() else {
        return Ok(options);
    };

    if encode_format != EncodeFormat::Xz {
        return Err(DiagnosticCause::from(Error::InvalidOption {
            message: "--filters is only supported with .xz output".into(),
        }));
    }

    let filters = parse_filters_chain(raw_filters, compression_level)?;
    options = options.with_filters(filters);
    Ok(options)
}

/// Apply `--threads` to compression options when supported by the container format.
fn apply_threads_for_compression(
    mut options: CompressionOptions,
    config: &CliConfig,
    encode_format: EncodeFormat,
) -> Result<CompressionOptions> {
    let Some(threads) = config.threads else {
        return Ok(options);
    };

    if matches!(encode_format, EncodeFormat::Lzma | EncodeFormat::Raw) {
        // `.lzma` is always single-threaded. Keep CLI compatibility by accepting `--threads`
        // but ignoring it for these single-threaded formats.
        return Ok(options);
    }

    let thread_count = u32::try_from(threads)
        .map_err(|_| DiagnosticCause::from(Error::InvalidThreadCount { count: threads }))?;
    options = options.with_threads(xz_core::Threading::Exact(thread_count));
    Ok(options)
}

/// Emit verbose/robot output for a completed compression operation.
fn emit_compress_summary(config: &CliConfig, bytes_read: u64, bytes_written: u64) {
    if !(config.verbose || config.robot) {
        return;
    }

    let ratio = ratio(bytes_written, bytes_read);
    if config.robot {
        println!("{bytes_read} {bytes_written} {ratio:.1}");
    } else {
        eprintln!("Compressed {bytes_read} bytes to {bytes_written} bytes ({ratio:.1}% ratio)");
    }
}

/// Compresses data from an input reader to an output writer.
///
/// Uses the XZ compression format with settings specified in [`CliConfig`].
/// Supports configurable compression levels (0-9), multi-threading, and
/// verbose progress reporting.
///
/// # Parameters
///
/// * `input` - Reader providing uncompressed data
/// * `output` - Writer receiving compressed XZ data
/// * `config` - CLI configuration specifying compression level, threads, and verbosity
///
/// # Returns
///
/// Returns `Ok(())` on successful compression.
///
/// # Errors
///
/// Returns an error in these cases:
///
/// - Invalid compression level (must be 0-9)
/// - Invalid thread count (too large for [`u32`])
/// - Compression operation failure from the underlying XZ library
/// - I/O errors during read or write operations
///
/// # Panics
///
/// This function does not panic during compression conversion.
pub fn compress_file(
    mut input: impl io::Read,
    mut output: impl io::Write,
    config: &CliConfig,
) -> Result<()> {
    let encode_format = resolve_encode_format(config);

    let compression_level = resolve_compression_level(config)?;

    let options = CompressionOptions::default()
        .with_format(encode_format)
        .with_check(config.check)
        .with_level(compression_level);
    let options = apply_lzma1_overrides(options, config, encode_format, compression_level)?;
    let options = apply_lzma2_overrides(options, config, encode_format, compression_level)?;
    let options = apply_filters_override(options, config, encode_format, compression_level)?;
    let options = apply_threads_for_compression(options, config, encode_format)?;

    // Perform compression and handle errors
    let summary = compress(&mut input, &mut output, &options).map_err(|e| {
        let message = xz_message_from_core_error(&e);
        DiagnosticCause::from(Error::Compression { message })
    })?;

    emit_compress_summary(config, summary.bytes_read, summary.bytes_written);

    Ok(())
}

/// Emit verbose/robot output for a completed decompression operation.
fn emit_decompress_summary(config: &CliConfig, bytes_read: u64, bytes_written: u64) {
    if !(config.verbose || config.robot) {
        return;
    }

    let ratio = ratio(bytes_written, bytes_read);
    if config.robot {
        println!("{bytes_read} {bytes_written} {ratio:.1}");
    } else {
        eprintln!(
            "Decompressed {bytes_read} bytes to {bytes_written} bytes ({ratio:.1}% expansion)"
        );
    }
}

/// Apply `--memlimit` to decompression options when a nonzero limit is configured.
fn apply_memlimit(mut options: DecompressionOptions, config: &CliConfig) -> DecompressionOptions {
    if let Some(memory_limit) = config.memory_limit {
        if let Some(limit) = std::num::NonZeroU64::new(memory_limit) {
            options = options.with_memlimit(limit);
        }
    }
    options
}

/// Apply `--threads` to decompression options (skipped for LZMA streams).
fn apply_threads_for_decompression(
    mut options: DecompressionOptions,
    config: &CliConfig,
) -> Result<DecompressionOptions> {
    let Some(threads) = config.threads else {
        return Ok(options);
    };

    if config.format == xz_core::config::DecodeMode::Lzma {
        return Ok(options);
    }

    let thread_count = u32::try_from(threads)
        .map_err(|_| DiagnosticCause::from(Error::InvalidThreadCount { count: threads }))?;
    options = options.with_threads(xz_core::Threading::Exact(thread_count));
    Ok(options)
}

/// Build decoder flags from CLI configuration.
fn build_decoder_flags(config: &CliConfig) -> Flags {
    let mut flags = if config.single_stream {
        Flags::empty()
    } else {
        Flags::CONCATENATED
    };

    if config.ignore_check {
        flags |= Flags::IGNORE_CHECK;
    }

    flags
}

/// Decompress a raw LZMA stream (no container framing).
fn decompress_raw(
    input: &mut impl io::Read,
    output: &mut impl io::Write,
    config: &CliConfig,
) -> Result<()> {
    let Some(raw_lzma1) = config.lzma1.as_deref() else {
        return Err(DiagnosticCause::from(Error::InvalidOption {
            message: "--format=raw requires --lzma1 filter options".into(),
        }));
    };

    let lzma1 = build_lzma1_options(raw_lzma1, Compression::default())?;

    let mut options = DecompressionOptions::default()
        .with_mode(config.format)
        .with_raw_lzma1_options(Some(lzma1));

    options = apply_memlimit(options, config);

    let outcome = decompress(input, output, &options).map_err(|e| {
        let message = xz_message_from_core_error(&e);
        DiagnosticCause::from(Error::Decompression { message })
    })?;

    emit_decompress_summary(config, outcome.bytes_read, outcome.bytes_written);
    Ok(())
}

/// Emit an unsupported integrity-check warning when applicable.
fn warn_unsupported_check(unsupported_check_id: Option<u32>, config: &CliConfig) -> Result<()> {
    if let Some(check_id) = unsupported_check_id {
        if !config.no_warn {
            return Err(DiagnosticCause::from(Warning::UnsupportedCheck {
                check_id,
            }));
        }
    }
    Ok(())
}

/// Decompresses XZ or LZMA data from an input reader to an output writer.
///
/// Automatically detects the compression format (XZ or LZMA) and decompresses
/// accordingly.
///
/// # Parameters
///
/// * `input` - Reader providing compressed XZ or LZMA data
/// * `output` - Writer receiving decompressed data
/// * `config` - CLI configuration specifying threads, memory limits, and verbosity
/// * `stdin_input` - Indicates that the current input source is standard input
///
/// # Returns
///
/// Returns `Ok(())` on successful decompression.
///
/// # Errors
///
/// Returns an error in these cases:
///
/// - Invalid thread count (too large for [`u32`])
/// - Corrupted or invalid input data
/// - Memory limit exceeded during decompression
/// - Decompression operation failure from the underlying XZ library
/// - I/O errors during read or write operations
pub fn decompress_file(
    mut input: impl io::Read,
    mut output: impl io::Write,
    config: &CliConfig,
    stdin_input: bool,
) -> Result<()> {
    if config.format == xz_core::config::DecodeMode::Raw {
        return decompress_raw(&mut input, &mut output, config);
    }

    let unknown_input_policy = if config.mode == crate::config::OperationMode::Decompress
        && config.stdout
        && config.format == xz_core::config::DecodeMode::Auto
        && stdin_input
    {
        // Mirror upstream `xz`: when reading from stdin in `xz -dc`-style
        // invocation, unknown input is copied to stdout unchanged.
        UnknownInputPolicy::Passthrough
    } else {
        // For named files and all other modes, unknown input must be
        // treated as an error so that corrupted `.xz` files (including
        // those with invalid header magic) cause a non-zero exit status.
        UnknownInputPolicy::Error
    };

    let options = DecompressionOptions::default()
        .with_mode(config.format)
        .with_flags(build_decoder_flags(config))
        .with_unknown_input_policy(unknown_input_policy);
    let options = apply_threads_for_decompression(options, config)?;
    let options = apply_memlimit(options, config);

    let outcome = decompress(&mut input, &mut output, &options).map_err(|e| {
        let message = xz_message_from_core_error(&e);
        DiagnosticCause::from(Error::Decompression { message })
    })?;

    emit_decompress_summary(config, outcome.bytes_read, outcome.bytes_written);

    warn_unsupported_check(outcome.unsupported_check_id, config)
}

/// Lists information about an XZ compressed file.
///
/// Extracts and displays metadata about the compressed file including:
///
/// - Number of streams and blocks
/// - Compressed and uncompressed sizes
/// - Compression ratio
/// - Integrity check types
///
/// In verbose mode, shows detailed information about each stream and block.
///
/// # Parameters
///
/// * `input_path` - Path to the input XZ file
/// * `config` - CLI configuration specifying verbosity and output format
///
/// # Returns
///
/// Returns `Ok(())` if the file was successfully analyzed.
///
/// # Errors
///
/// Returns an error if:
///
/// - The file cannot be opened or read
/// - The file is not a valid XZ file
/// - Memory limit is exceeded during analysis
pub fn list_file(input_path: &Path, config: &CliConfig) -> Result<()> {
    let ctx = ListOutputContext {
        file_index: 1,
        file_count: 1,
        print_header: !config.robot && !config.verbose,
    };

    let _ = list_file_with_context(input_path, config, ctx)?;
    Ok(())
}

/// Lists information about an XZ compressed file with context for multi-file processing.
///
/// # Parameters
///
/// * `input_path` - Path to the input XZ file
/// * `config` - CLI configuration specifying verbosity and output format
/// * `ctx` - Output context for multi-file formatting (file index, count, header printing)
///
/// # Returns
///
/// Returns `Ok(ListSummary)` with file metadata if the file was successfully analyzed.
///
/// # Errors
///
/// Returns an error if:
///
/// - The file cannot be opened or read
/// - The file is not a valid XZ file
/// - Memory limit is exceeded during analysis
/// - Writing to stdout fails (e.g., broken pipe)
pub fn list_file_with_context(
    input_path: &Path,
    config: &CliConfig,
    ctx: ListOutputContext,
) -> Result<ListSummary> {
    if input_path.as_os_str().is_empty() || input_path == Path::new("-") {
        return Err(DiagnosticCause::from(Error::ListModeStdinUnsupported));
    }

    let mut file = File::open(input_path).map_err(|source| {
        DiagnosticCause::from(Error::OpenInput {
            source: IoErrorNoCode::new(source),
        })
    })?;

    // Extract file info
    let memlimit = config.memory_limit.and_then(std::num::NonZeroU64::new);
    let info = file_info::extract_file_info(&mut file, memlimit).map_err(|e| {
        DiagnosticCause::from(Error::FileInfoExtraction {
            path: input_path.display().to_string(),
            message: e.to_string(),
        })
    })?;

    let summary = ListSummary {
        stream_count: info.stream_count(),
        block_count: info.block_count(),
        compressed: info.file_size(),
        uncompressed: info.uncompressed_size(),
        checks_mask: info.checks(),
    };

    if config.robot {
        use std::io::Write;

        // Machine-readable output
        let mut out = io::stdout().lock();
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{:.3}\t{}",
            input_path.display(),
            info.stream_count(),
            info.block_count(),
            info.file_size(),
            info.uncompressed_size(),
            ratio(info.file_size(), info.uncompressed_size())
        )
        .map_err(|source| {
            DiagnosticCause::from(Error::WriteOutput {
                source: IoErrorNoCode::new(source),
            })
        })?;
    } else if config.verbose {
        let streams = info.streams();
        let mut blocks = info.blocks();
        blocks.sort_by_key(|b| b.number_in_file);
        list::write_verbose_report(input_path, ctx, summary, &streams, &blocks)?;
    } else {
        list::write_list_header_if_needed(ctx)?;
        list::write_list_row(summary, input_path)?;
    }

    Ok(summary)
}
