//! Compression and decompression operations for XZ CLI.

use std::fs::File;
use std::io;
use std::io::Read as _;

use xz_core::{
    config::EncodeFormat,
    detect_unsupported_xz_check_id, file_info,
    options::lzma1::Lzma1Options,
    options::{Compression, CompressionOptions, DecompressionOptions, Flags},
    pipeline::{compress, decompress},
    ratio, read_xz_stream_header_prefix, Error as CoreError,
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

/// Apply `--lzma1` overrides to compression options when encoding the `.lzma` container.
fn apply_lzma1_overrides(
    mut options: CompressionOptions,
    config: &CliConfig,
    encode_format: EncodeFormat,
    compression_level: Compression,
) -> Result<CompressionOptions> {
    let Some(raw_lzma1) = config.lzma1.as_deref() else {
        return Ok(options);
    };

    if encode_format != EncodeFormat::Lzma {
        return Err(DiagnosticCause::from(Error::InvalidOption {
            message: "--lzma1 is only supported with --format=lzma".into(),
        }));
    }

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

    options = options.with_lzma1_options(Some(lzma1));
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

    if encode_format == EncodeFormat::Lzma {
        // `.lzma` is always single-threaded. Keep CLI compatibility by accepting `--threads`
        // but ignoring it for this container format.
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
    let options = apply_threads_for_compression(options, config, encode_format)?;

    // Perform compression and handle errors
    let summary = compress(&mut input, &mut output, &options).map_err(|e| {
        let message = xz_message_from_core_error(&e);
        DiagnosticCause::from(Error::Compression { message })
    })?;

    emit_compress_summary(config, summary.bytes_read, summary.bytes_written);

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
) -> Result<()> {
    // Read and preserve a small prefix so we can inspect the XZ Stream Header without
    // losing bytes from the actual decode stream.
    let prefix = read_xz_stream_header_prefix(&mut input).map_err(|source| {
        DiagnosticCause::from(Error::OpenInput {
            source: IoErrorNoCode::new(source),
        })
    })?;

    let unsupported_check_id = detect_unsupported_xz_check_id(&prefix);

    let mut input = io::Cursor::new(prefix).chain(input);

    let mut options = DecompressionOptions::default();

    // Set thread count if specified
    if let Some(threads) = config.threads {
        if config.format != xz_core::config::DecodeMode::Lzma {
            let thread_count = u32::try_from(threads)
                .map_err(|_| DiagnosticCause::from(Error::InvalidThreadCount { count: threads }))?;
            options = options.with_threads(xz_core::Threading::Exact(thread_count));
        }
    }

    // Set memory limit if specified
    if let Some(memory_limit) = config.memory_limit {
        // Only set if memory_limit is nonzero
        if let Some(limit) = std::num::NonZeroU64::new(memory_limit) {
            options = options.with_memlimit(limit);
        }
    }

    // Set decode mode based on format
    options = options.with_mode(config.format);

    // Configure decoder flags
    //
    // Build flags based on configuration:
    // - CONCATENATED: Process multiple concatenated streams (default)
    // - IGNORE_CHECK: Skip integrity check verification (when requested)
    let mut flags = if config.single_stream {
        Flags::empty()
    } else {
        Flags::CONCATENATED
    };

    if config.ignore_check {
        flags |= Flags::IGNORE_CHECK;
    }

    options = options.with_flags(flags);

    // Perform decompression and handle errors
    let summary = decompress(&mut input, &mut output, &options).map_err(|e| {
        let message = xz_message_from_core_error(&e);
        DiagnosticCause::from(Error::Decompression { message })
    })?;

    // Print output if verbose or robot mode is enabled
    if config.verbose || config.robot {
        let ratio = ratio(summary.bytes_written, summary.bytes_read);

        if config.robot {
            // Machine-readable output for robot mode
            println!(
                "{} {} {:.1}",
                summary.bytes_read, summary.bytes_written, ratio
            );
        } else {
            // Human-readable output for verbose mode
            eprintln!(
                "Decompressed {} bytes to {} bytes ({:.1}% expansion)",
                summary.bytes_read, summary.bytes_written, ratio
            );
        }
    }

    if let Some(check_id) = unsupported_check_id {
        if !config.no_warn {
            // Decoding can succeed but unsupported integrity
            // check type must be reported as a warning (exit code 2).
            return Err(DiagnosticCause::from(Warning::UnsupportedCheck {
                check_id,
            }));
        }
    }

    Ok(())
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
pub fn list_file(input_path: &str, config: &CliConfig) -> Result<()> {
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
pub(crate) fn list_file_with_context(
    input_path: &str,
    config: &CliConfig,
    ctx: ListOutputContext,
) -> Result<ListSummary> {
    if input_path.is_empty() || input_path == "-" {
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
            path: input_path.to_string(),
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
            input_path,
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
