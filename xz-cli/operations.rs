//! Compression and decompression operations for XZ CLI.

use std::io;

use xz_core::{
    options::{CompressionOptions, DecompressionOptions},
    pipeline::{compress, decompress},
};

use crate::config::CliConfig;
use crate::error::{Error, Result};

/// Calculates the compression/decompression ratio as a percentage.
///
/// # Parameters
///
/// * `numerator` - Output byte count
/// * `denominator` - Input byte count
///
/// # Returns
///
/// The ratio as a percentage (0.0-100.0+), or 0.0 if denominator is zero.
fn calculate_ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator > 0 {
        // Use integer division and remainder to avoid direct u64 -> f64 cast
        // This maintains better precision for large values
        let quotient = numerator / denominator;
        let remainder = numerator % denominator;

        f64::from(u32::try_from(quotient).unwrap_or(u32::MAX)) * 100.0
            + (f64::from(u32::try_from(remainder).unwrap_or(u32::MAX))
                / f64::from(u32::try_from(denominator).unwrap_or(u32::MAX)))
                * 100.0
    } else {
        0.0
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

    // Perform compression and handle errors
    let summary = compress(&mut input, &mut output, &options).map_err(|e| Error::Compression {
        path: "(input)".to_string(),
        message: e.to_string(),
    })?;

    // Print verbose output if enabled
    if config.verbose {
        let ratio = calculate_ratio(summary.bytes_written, summary.bytes_read);

        eprintln!(
            "Compressed {} bytes to {} bytes ({:.1}% ratio)",
            summary.bytes_read, summary.bytes_written, ratio
        );
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

    // Perform decompression and handle errors
    let summary =
        decompress(&mut input, &mut output, &options).map_err(|e| Error::Decompression {
            path: "(input)".to_string(),
            message: e.to_string(),
        })?;

    // Print verbose output if enabled
    if config.verbose {
        let ratio = calculate_ratio(summary.bytes_written, summary.bytes_read);

        eprintln!(
            "Decompressed {} bytes to {} bytes ({:.1}% expansion)",
            summary.bytes_read, summary.bytes_written, ratio
        );
    }

    Ok(())
}
