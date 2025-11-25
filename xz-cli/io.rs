//! File I/O operations and path manipulation for XZ CLI.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{CliConfig, OperationMode, DEFAULT_BUFFER_SIZE, LZMA_EXTENSION, XZ_EXTENSION};
use crate::error::{Error, Result};

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
/// Returns an error in these cases:
///
/// - Decompression mode: Input file lacks a recognized compression extension
/// - Decompression mode: Cannot determine a valid file stem from the input path
pub fn generate_output_filename(input: &Path, mode: OperationMode) -> Result<PathBuf> {
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
                return Err(Error::InvalidExtension {
                    path: input.to_path_buf(),
                });
            }
            // Get the file stem (filename without last extension)
            let stem = input
                .file_stem()
                .ok_or_else(|| Error::InvalidOutputFilename {
                    path: input.to_path_buf(),
                })?;

            // Use the parent directory, or current directory if none
            let parent = input.parent().unwrap_or_else(|| Path::new("."));
            Ok(parent.join(stem))
        }
        // No output file for test mode or list mode
        OperationMode::Test | OperationMode::List => Ok(PathBuf::new()),
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
/// Returns an error if the file cannot be opened.
pub fn open_input(path: &str) -> Result<Box<dyn io::Read>> {
    if path.is_empty() {
        Ok(Box::new(io::BufReader::with_capacity(
            DEFAULT_BUFFER_SIZE,
            io::stdin(),
        )))
    } else {
        let file = File::open(path).map_err(|source| Error::OpenInput {
            path: path.to_string(),
            source,
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
/// Returns an error in the following cases:
///
/// - The output file already exists and `config.force` is `false`
/// - The file cannot be created due to permissions, disk space, etc.
pub fn open_output(path: Option<&Path>, config: &CliConfig) -> Result<Box<dyn io::Write>> {
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
            return Err(Error::OutputExists {
                path: path.to_path_buf(),
            });
        }
        let file = File::create(path).map_err(|source| Error::CreateOutput {
            path: path.to_path_buf(),
            source,
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
