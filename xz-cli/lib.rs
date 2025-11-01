//! Common CLI utilities and shared functionality for XZ command-line tools.
//!
//! This module provides high-level abstractions for XZ compression and decompression
//! operations, file I/O handling, and CLI configuration management. It serves as the
//! primary interface between command-line tools and the core XZ functionality.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use xz_core::{
    options::{CompressionOptions, DecompressionOptions},
    pipeline::{compress, decompress},
};

#[cfg(test)]
mod tests;

/// Default buffer size for file I/O operations
pub const DEFAULT_BUFFER_SIZE: usize = 512 * 1024;

/// File extension for XZ compressed files
pub const XZ_EXTENSION: &str = "xz";

/// File extension for LZMA compressed files
pub const LZMA_EXTENSION: &str = "lzma";

/// Represents different modes of operation for CLI utilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    /// Compress input data
    Compress,
    /// Decompress input data
    Decompress,
    /// Decompress and output to stdout (like cat)
    Cat,
    /// Test integrity without extracting
    Test,
}

/// Configuration for CLI operations
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct CliConfig {
    /// Operation mode
    pub mode: OperationMode,
    /// Force overwrite existing files
    pub force: bool,
    /// Keep input files after processing
    pub keep: bool,
    /// Output to stdout
    pub stdout: bool,
    /// Verbose output
    pub verbose: bool,
    /// Compression level (0-9)
    pub level: Option<u32>,
    /// Number of threads to use
    pub threads: Option<usize>,
    /// Memory limit for decompression
    pub memory_limit: Option<u64>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            mode: OperationMode::Compress,
            force: false,
            keep: false,
            stdout: false,
            verbose: false,
            level: None,
            threads: None,
            memory_limit: None,
        }
    }
}

/// Checks if a file path has a recognized compression extension.
///
/// Recognizes `.xz` and `.lzma` extensions (case-insensitive).
///
/// # Parameters
///
/// * `path` - The file path to check
///
/// # Returns
///
/// Returns `true` if the file has a `.xz` or `.lzma` extension, `false` otherwise.
pub fn has_compression_extension(path: &Path) -> bool {
    // Get the extension as a lowercase string, if present
    if let Some(ext) = path.extension().and_then(OsStr::to_str) {
        let ext = ext.to_ascii_lowercase();
        ext == XZ_EXTENSION || ext == LZMA_EXTENSION
    } else {
        false
    }
}

/// Generates an output filename based on input path and operation mode.
///
/// # Parameters
///
/// * `input` - The input file path
/// * `mode` - The operation mode
///
/// # Returns
///
/// The generated output path.
///
/// # Errors
///
/// Returns an [`io::Error`] with kind [`io::ErrorKind::InvalidInput`] in these cases:
///
/// - Decompression mode: Input file lacks a recognized compression extension
/// - Decompression mode: Cannot determine a valid file stem from the input path
pub fn generate_output_filename(input: &Path, mode: OperationMode) -> io::Result<PathBuf> {
    match mode {
        OperationMode::Compress => {
            let mut output = input.to_path_buf();
            // If the file has an extension, append .xz after it, otherwise just set .xz
            match input
                .extension()
                .and_then(OsStr::to_str)
                .filter(|ext| !ext.is_empty())
            {
                Some(ext) => {
                    let new_ext = format!("{ext}.{XZ_EXTENSION}");
                    output.set_extension(new_ext);
                }
                None => {
                    output.set_extension(XZ_EXTENSION);
                }
            }
            Ok(output)
        }
        OperationMode::Decompress | OperationMode::Cat => {
            // Ensure the input file has a recognized compression extension
            if !has_compression_extension(input) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Input file '{}' does not have a recognized compression extension",
                        input.display()
                    ),
                ));
            }
            // Get the file stem (filename without last extension)
            let stem = input.file_stem().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Cannot determine output filename for '{}'", input.display()),
                )
            })?;

            // Use the parent directory, or current directory if none
            let parent = input.parent().unwrap_or_else(|| Path::new("."));
            Ok(parent.join(stem))
        }
        // No output file for test mode
        OperationMode::Test => Ok(PathBuf::new()),
    }
}

/// Opens an input reader for the given path, or stdin if path is empty.
///
/// # Parameters
///
/// * `path` - Path to the input file, or empty string for stdin
///
/// # Returns
///
/// A trait object implementing [`io::Read`] that wraps either:
///
/// - A buffered file reader for non-empty paths
/// - A buffered stdin reader for empty paths
///
/// # Errors
///
/// Returns an [`io::Error`] if the file cannot be opened.
pub fn open_input(path: &str) -> io::Result<Box<dyn io::Read>> {
    if path.is_empty() {
        Ok(Box::new(io::BufReader::with_capacity(
            DEFAULT_BUFFER_SIZE,
            io::stdin(),
        )))
    } else {
        let file = File::open(path).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Failed to open input file '{path}'"),
            )
        })?;
        Ok(Box::new(io::BufReader::with_capacity(
            DEFAULT_BUFFER_SIZE,
            file,
        )))
    }
}

/// Opens an output writer for the given path or stdout.
///
/// # Parameters
///
/// * `path` - Optional path to the output file. If `None`, writes to stdout
/// * `config` - CLI configuration controlling stdout mode and force overwrite
///
/// # Returns
///
/// A trait object implementing [`io::Write`] that wraps either:
///
/// - A buffered file writer for file output
/// - A buffered stdout writer for stdout output
///
/// # Errors
///
/// Returns an [`io::Error`] in the following cases:
///
/// - The output file already exists and `config.force` is `false`
///   (returns [`io::ErrorKind::AlreadyExists`])
/// - The file cannot be created due to permissions, disk space, etc.
pub fn open_output(path: Option<&Path>, config: &CliConfig) -> io::Result<Box<dyn io::Write>> {
    // Determine if we should write to stdout
    let use_stdout = config.stdout || path.is_none_or(|p| p.as_os_str().is_empty());

    if use_stdout {
        Ok(Box::new(io::BufWriter::with_capacity(
            DEFAULT_BUFFER_SIZE,
            io::stdout(),
        )))
    } else if let Some(path) = path {
        // Check if output file exists and we're not forcing overwrite
        if path.exists() && !config.force {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "Output file '{}' already exists. Use --force to overwrite.",
                    path.display()
                ),
            ));
        }
        let file = File::create(path).map_err(|_| {
            io::Error::other(format!("Failed to create output file '{}'", path.display()))
        })?;

        Ok(Box::new(io::BufWriter::with_capacity(
            DEFAULT_BUFFER_SIZE,
            file,
        )))
    } else {
        // Fallback to stdout if no path is provided
        Ok(Box::new(io::BufWriter::with_capacity(
            DEFAULT_BUFFER_SIZE,
            io::stdout(),
        )))
    }
}

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
/// Returns an [`io::Error`] in these cases:
///
/// - Invalid compression level (must be 0-9)
/// - Invalid thread count (too large for [`u32`])
/// - Compression operation failure from the underlying XZ library
/// - I/O errors during read or write operations
pub fn compress_file(
    mut input: impl io::Read,
    mut output: impl io::Write,
    config: &CliConfig,
) -> io::Result<()> {
    let mut options = CompressionOptions::default();

    // Set compression level if specified
    if let Some(level) = config.level {
        let compression_level = xz_core::options::Compression::try_from(level).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid compression level: {level}"),
            )
        })?;
        options = options.with_level(compression_level);
    }

    // Set thread count if specified
    if let Some(threads) = config.threads {
        let thread_count = u32::try_from(threads).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Thread count {threads} is too large"),
            )
        })?;
        options = options.with_threads(xz_core::Threading::Exact(thread_count));
    }

    // Perform compression and handle errors
    let summary = compress(&mut input, &mut output, &options)
        .map_err(|e| io::Error::other(format!("Compression failed: {e}")))?;

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
/// Returns an [`io::Error`] in these cases:
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
) -> io::Result<()> {
    let mut options = DecompressionOptions::default();

    // Set thread count if specified
    if let Some(threads) = config.threads {
        let thread_count = u32::try_from(threads).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Thread count {threads} is too large"),
            )
        })?;
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
    let summary = decompress(&mut input, &mut output, &options).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Decompression failed: {e}"),
        )
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

/// Removes the input file after successful processing.
///
/// Automatically determines whether to remove the input file based on the
/// operation mode and configuration flags.
///
/// # Parameters
///
/// * `input_path` - Path to the input file to potentially remove (empty string for stdin)
/// * `config` - CLI configuration controlling file retention behavior
///
/// # Returns
///
/// Returns `Ok(())` if the file was removed or if removal was not necessary.
///
/// # Errors
///
/// Returns an [`io::Error`] if file removal fails.
pub fn cleanup_input_file(input_path: &str, config: &CliConfig) -> io::Result<()> {
    // Never delete input file in Test mode
    if config.mode == OperationMode::Test {
        return Ok(());
    }

    if !config.keep && !input_path.is_empty() && !config.stdout {
        std::fs::remove_file(input_path).map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("Failed to remove input file '{input_path}': {err}"),
            )
        })?;

        if config.verbose {
            eprintln!("Removed input file: {input_path}");
        }
    }
    Ok(())
}

/// Processes a single file according to the CLI configuration.
///
/// This is the main entry point for file processing operations. It orchestrates
/// the complete workflow:
///
/// 1. Opens the input file (or stdin if path is empty)
/// 2. Generates the output filename (if needed)
/// 3. Opens the output destination (file or stdout)
/// 4. Performs the requested operation (compress/decompress/cat/test)
/// 5. Cleans up the input file if configured to do so
///
/// # Parameters
///
/// * `input_path` - Path to the input file, or empty string to read from stdin
/// * `config` - CLI configuration specifying operation mode, levels, and flags
///
/// # Operation Modes
///
/// - **Compress**: Reads uncompressed data and writes XZ-compressed output
/// - **Decompress**: Reads XZ/LZMA data and writes decompressed output
/// - **Cat**: Like decompress but always writes to stdout
/// - **Test**: Validates compressed data integrity without producing output
///
/// # Returns
///
/// Returns `Ok(())` if the operation completed successfully.
///
/// # Errors
///
/// Returns an [`io::Error`] in these cases:
///
/// - Input file cannot be opened or read
/// - Output filename generation fails (e.g., decompressing file without valid extension)
/// - Output file creation fails (permissions, disk space, etc.)
/// - Output file exists and `force` flag is not set
/// - Compression/decompression operation fails
/// - Input file removal fails (when cleanup is enabled)
pub fn process_file(input_path: &str, config: &CliConfig) -> io::Result<()> {
    // Use empty PathBuf for stdin, otherwise use the provided path
    let input_path_buf = if input_path.is_empty() {
        PathBuf::new()
    } else {
        PathBuf::from(input_path)
    };

    let input = open_input(input_path)?;

    // Determine output path
    let output_path =
        if config.stdout || config.mode == OperationMode::Cat || config.mode == OperationMode::Test
        {
            None
        } else {
            Some(generate_output_filename(&input_path_buf, config.mode)?)
        };

    // Open output
    let output = open_output(output_path.as_deref(), config)?;

    // Process based on mode
    match config.mode {
        OperationMode::Compress => {
            compress_file(input, output, config)?;
        }
        OperationMode::Decompress | OperationMode::Cat => {
            decompress_file(input, output, config)?;
        }
        OperationMode::Test => {
            // In test mode, decompress but discard output
            decompress_file(input, io::sink(), config)?;
            if config.verbose {
                eprintln!("Test successful: {input_path}");
            }
        }
    }

    // Remove input file if allowed
    cleanup_input_file(input_path, config)?;

    Ok(())
}

/// Parses a memory limit string with an optional size suffix.
///
/// Accepts numeric values with optional suffixes: `K` (KiB), `M` (MiB), or `G` (GiB).
/// All suffixes are case-insensitive. Values without a suffix are interpreted as bytes.
///
/// # Parameters
///
/// * `s` - The memory limit string to parse (e.g., "1024", "1K", "512M", "2G")
///
/// # Returns
///
/// The memory limit in bytes as a [`u64`].
///
/// # Errors
///
/// Returns an error in the following cases:
///
/// - The input string is empty
/// - The numeric part cannot be parsed as a valid [`u64`]
/// - The suffix is not one of K, M, G, or a digit
/// - The result would overflow [`u64`] after applying the multiplier
pub fn parse_memory_limit(s: &str) -> Result<u64, String> {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    let s = s.trim();
    if s.is_empty() {
        return Err("Empty memory limit".to_string());
    }

    let (number_part, multiplier) = if let Some(last_char) = s.chars().last() {
        match last_char.to_ascii_uppercase() {
            'K' => (&s[..s.len() - 1], KB),
            'M' => (&s[..s.len() - 1], MB),
            'G' => (&s[..s.len() - 1], GB),
            _ if last_char.is_ascii_digit() => (s, 1),
            _ => return Err(format!("Invalid memory limit suffix: {last_char}")),
        }
    } else {
        (s, 1)
    };

    let number: u64 = number_part
        .parse()
        .map_err(|_| format!("Invalid memory limit number: {number_part}"))?;

    number
        .checked_mul(multiplier)
        .ok_or_else(|| "Memory limit too large".to_string())
}

/// Runs a CLI command over multiple input files with error context.
///
/// This is a convenience wrapper around [`process_file`] that processes multiple
/// files sequentially and provides enhanced error messages with program name and
/// file context.
///
/// # Parameters
///
/// * `files` - Slice of input file paths to process. Empty slice reads from stdin.
/// * `config` - CLI configuration specifying operation mode and options.
/// * `program` - Program name to include in error messages (e.g., "xz", "unxz").
///
/// # Returns
///
/// Returns `Ok(())` if all files were processed successfully.
///
/// # Errors
///
/// Returns an error if any file operation fails. The error message includes
/// the program name and file path for better error reporting. Processing stops
/// at the first error
pub fn run_cli(files: &[String], config: &CliConfig, program: &str) -> io::Result<()> {
    if files.is_empty() {
        process_file("", config).map_err(|_| io::Error::other(format!("{program}: (stdin)")))?;
    } else {
        for file in files {
            process_file(file, config)
                .map_err(|err| io::Error::other(format!("{program}: {file}: {err}")))?;
        }
    }

    Ok(())
}
