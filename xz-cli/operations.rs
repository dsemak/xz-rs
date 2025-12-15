//! Compression and decompression operations for XZ CLI.

use std::fs::File;
use std::io;

use xz_core::{
    file_info,
    options::{Compression, CompressionOptions, DecompressionOptions, Flags},
    pipeline::{compress, decompress},
    ratio,
};

use crate::config::CliConfig;
use crate::error::{DiagnosticCause, Error, Result};
use crate::format::list::{self, ListOutputContext, ListSummary};

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
pub fn compress_file(
    mut input: impl io::Read,
    mut output: impl io::Write,
    config: &CliConfig,
) -> Result<()> {
    let mut options = CompressionOptions::default();

    // Determine the compression level and apply extreme mode if enabled
    //
    // Extreme is a modifier that applies to the selected preset level,
    // not a separate level.
    let compression_level = match (config.level, config.extreme) {
        (Some(level), true) => Compression::Extreme(level as u8),
        (Some(level), false) => Compression::try_from(level)
            .map_err(|_| DiagnosticCause::from(Error::InvalidCompressionLevel { level }))?,
        (None, true) => Compression::Extreme(Compression::default().to_preset() as u8),
        (None, false) => Compression::default(),
    };

    options = options.with_level(compression_level);

    // Set thread count if specified
    if let Some(threads) = config.threads {
        let thread_count = u32::try_from(threads)
            .map_err(|_| DiagnosticCause::from(Error::InvalidThreadCount { count: threads }))?;
        options = options.with_threads(xz_core::Threading::Exact(thread_count));
    }

    // Perform compression and handle errors
    let summary = compress(&mut input, &mut output, &options).map_err(|e| {
        DiagnosticCause::from(Error::Compression {
            path: "(input)".to_string(),
            message: e.to_string(),
        })
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
                "Compressed {} bytes to {} bytes ({:.1}% ratio)",
                summary.bytes_read, summary.bytes_written, ratio
            );
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
    let mut options = DecompressionOptions::default();

    // Set thread count if specified
    if let Some(threads) = config.threads {
        let thread_count = u32::try_from(threads)
            .map_err(|_| DiagnosticCause::from(Error::InvalidThreadCount { count: threads }))?;
        options = options.with_threads(xz_core::Threading::Exact(thread_count));
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
        DiagnosticCause::from(Error::Decompression {
            path: "(input)".to_string(),
            message: e.to_string(),
        })
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
            path: input_path.to_string(),
            source,
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
        .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
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
