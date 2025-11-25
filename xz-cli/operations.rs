//! Compression and decompression operations for XZ CLI.

use std::fs::File;
use std::io;

use xz_core::{
    file_info,
    options::{Compression, CompressionOptions, DecompressionOptions},
    pipeline::{compress, decompress},
    ratio,
};

use crate::config::CliConfig;
use crate::error::{Error, Result};

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

    // Set compression level if specified
    if let Some(level) = config.level {
        let compression_level = xz_core::options::Compression::try_from(level)
            .map_err(|_| Error::InvalidCompressionLevel { level })?;
        options = options.with_level(compression_level);
    }

    // Set thread count if specified
    if let Some(threads) = config.threads {
        let thread_count =
            u32::try_from(threads).map_err(|_| Error::InvalidThreadCount { count: threads })?;
        options = options.with_threads(xz_core::Threading::Exact(thread_count));
    }

    // Set extreme mode if enabled
    if config.extreme {
        // In extreme mode, we use the highest compression level
        options = options.with_level(Compression::Level9);
    }

    // Perform compression and handle errors
    let summary = compress(&mut input, &mut output, &options).map_err(|e| Error::Compression {
        path: "(input)".to_string(),
        message: e.to_string(),
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
        let thread_count =
            u32::try_from(threads).map_err(|_| Error::InvalidThreadCount { count: threads })?;
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

    // Perform decompression and handle errors
    let summary =
        decompress(&mut input, &mut output, &options).map_err(|e| Error::Decompression {
            path: "(input)".to_string(),
            message: e.to_string(),
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
    // Open the file
    let mut file = File::open(input_path).map_err(|source| Error::OpenInput {
        path: input_path.to_string(),
        source,
    })?;

    // Extract file info
    let memlimit = config.memory_limit.and_then(std::num::NonZeroU64::new);
    let info = file_info::extract_file_info(&mut file, memlimit).map_err(|e| {
        Error::FileInfoExtraction {
            path: input_path.to_string(),
            message: e.to_string(),
        }
    })?;

    if config.robot {
        // Machine-readable output
        println!(
            "{}\t{}\t{}\t{}\t{:.3}\t{}",
            input_path,
            info.stream_count(),
            info.block_count(),
            info.file_size(),
            info.uncompressed_size(),
            ratio(info.file_size(), info.uncompressed_size())
        );
    } else if config.verbose {
        // Detailed human-readable output
        println!("File: {}", input_path);
        println!("  Streams:           {}", info.stream_count());
        println!("  Blocks:            {}", info.block_count());
        println!("  Compressed size:   {} bytes", info.file_size());
        println!("  Uncompressed size: {} bytes", info.uncompressed_size());
        println!(
            "  Ratio:             {:.2}%",
            ratio(info.file_size(), info.uncompressed_size())
        );
        println!("  Check:             0x{:08x}", info.checks());

        // Show detailed stream info
        for (idx, stream) in info.streams().iter().enumerate() {
            println!("\n  Stream {}:", idx + 1);
            println!("    Blocks:          {}", stream.block_count);
            println!("    Compressed:      {} bytes", stream.compressed_size);
            println!("    Uncompressed:    {} bytes", stream.uncompressed_size);
            println!(
                "    Ratio:           {:.2}%",
                ratio(stream.compressed_size, stream.uncompressed_size)
            );
            println!("    Padding:         {} bytes", stream.padding);
        }

        // Show detailed block info
        println!("\n  Blocks:");
        for block in info.blocks() {
            println!(
                "    Block {} (in stream {}):",
                block.number_in_file, block.number_in_stream
            );
            println!("      Compressed:    {} bytes", block.total_size);
            println!("      Uncompressed:  {} bytes", block.uncompressed_size);
            println!(
                "      Ratio:         {:.2}%",
                ratio(block.total_size, block.uncompressed_size) * 100.0
            );
        }
    } else {
        // Compact output
        println!(
            "{}: {} streams, {} blocks, {}/{} bytes, {:.1}%",
            input_path,
            info.stream_count(),
            info.block_count(),
            info.file_size(),
            info.uncompressed_size(),
            ratio(info.file_size(), info.uncompressed_size())
        );
    }

    Ok(())
}
