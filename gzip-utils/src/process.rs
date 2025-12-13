//! High-level file processing and CLI orchestration.

use std::io;
use std::path::PathBuf;

use crate::config::{CliConfig, OperationMode};
use crate::error::{Error, Result};
use crate::io::{generate_output_filename, open_input, open_output};
use crate::operations::{compress_file, decompress_file};

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
/// Returns an error if file removal fails.
pub fn cleanup_input_file(input_path: &str, config: &CliConfig) -> Result<()> {
    // Never delete input file in Test mode
    if config.mode == OperationMode::Test {
        return Ok(());
    }

    if !config.keep && !input_path.is_empty() && !config.stdout {
        std::fs::remove_file(input_path).map_err(|source| Error::RemoveFile {
            path: input_path.to_string(),
            source,
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
/// - **Decompress**: Reads XZ data and writes decompressed output
/// - **Cat**: Like decompress but always writes to stdout
/// - **Test**: Validates compressed data integrity without producing output
///
/// # Returns
///
/// Returns `Ok(())` if the operation completed successfully.
///
/// # Errors
///
/// Returns an error in these cases:
///
/// - Input file cannot be opened or read
/// - Output filename generation fails (e.g., decompressing file without valid extension)
/// - Output file creation fails (permissions, disk space, etc.)
/// - Output file exists and `force` flag is not set
/// - Compression/decompression operation fails
/// - Input file removal fails (when cleanup is enabled)
pub fn process_file(input_path: &str, config: &CliConfig) -> Result<()> {
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
