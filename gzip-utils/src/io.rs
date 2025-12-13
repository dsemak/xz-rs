//! File I/O operations and path manipulation for compression CLI.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::config::{OperationMode, GZIP_EXTENSION};
use crate::error::{Error, Result};

/// Checks if a file path has a gzip extension.
pub fn has_compression_extension(path: &Path) -> bool {
    // Get the extension as a lowercase string, if present
    if let Some(ext) = path.extension().and_then(OsStr::to_str) {
        let ext = ext.to_ascii_lowercase();
        ext == GZIP_EXTENSION
    } else {
        false
    }
}

/// Generates an output filename based on input path and operation mode.
pub fn generate_output_filename(input: &Path, mode: OperationMode) -> Result<PathBuf> {
    match mode {
        OperationMode::Compress => {
            let mut output = input.to_path_buf();
            
            // Preserve original extension and add .gz
            if let Some(ext) = input.extension().and_then(OsStr::to_str) {
                if !ext.is_empty() {
                    let new_ext = format!("{}.{}", ext, GZIP_EXTENSION);
                    output.set_extension(new_ext);
                } else {
                    output.set_extension(GZIP_EXTENSION);
                }
            } else {
                output.set_extension(GZIP_EXTENSION);
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

            // Geting full name without path
            let file_name = input.file_name()
                .ok_or_else(|| Error::InvalidOutputFilename { 
                    path: input.to_path_buf() 
                })?;

            // Convert to a string and drop .gz/.gzip
            let file_name_str = file_name.to_string_lossy();
            // Remove .gz or .gzip suffix
            let stem = file_name_str
                .trim_end_matches(".gz")
                .trim_end_matches(".GZ")
                .trim_end_matches(".gzip")
                .trim_end_matches(".GZIP");

            // If nothing remains after removing extension, use original
            let final_stem = if stem.is_empty() { &file_name_str } else { stem };

            // Use the parent directory, or current directory if none
            let parent = input.parent().unwrap_or_else(|| Path::new("."));
            Ok(parent.join(final_stem))
        }
        // No output file for test mode
        OperationMode::Test => Ok(PathBuf::new()),
    }
}

/// Opens an input reader for the given path, or stdin if path is empty.
pub fn open_input(path: &str) -> Result<Box<dyn Read>> {
    if path.is_empty() {
        Ok(Box::new(BufReader::new(io::stdin())))
    } else {
        let file = File::open(path).map_err(|source| Error::OpenInput {
            path: path.to_string(),
            source,
        })?;
        Ok(Box::new(BufReader::new(file)))
    }
}

/// Opens an output writer for the given path or stdout.
pub fn open_output(path: Option<&Path>, config: &crate::config::CliConfig,) -> Result<Box<dyn Write>> {
    let use_stdout = config.stdout || path.is_none_or(|p| p.as_os_str().is_empty());

    if use_stdout {
        Ok(Box::new(BufWriter::new(io::stdout())))
    } else if let Some(path) = path {
        if path.exists() && !config.force {
            return Err(Error::OutputExists {
                path: path.to_path_buf(),
            });
        }
        let file = File::create(path).map_err(|source| Error::CreateOutput {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Box::new(BufWriter::new(file)))
    } else {
        Ok(Box::new(BufWriter::new(io::stdout())))
    }
}
