//! File I/O operations and path manipulation for XZ/LZMA CLI.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::config::{CliConfig, CompressionFormat, OperationMode, XZ_EXTENSION, LZMA_EXTENSION};
use crate::error::{Error, Result};

/// Checks if a file path has a recognized XZ/LZMA extension
pub fn has_compression_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(OsStr::to_str) {
        let ext = ext.to_ascii_lowercase();
        ext == XZ_EXTENSION || ext == LZMA_EXTENSION
    } else {
        false
    }
}

/// Gets the appropriate extension based on configuration
fn get_extension(config: &CliConfig) -> &'static str {
    match config.format {
        CompressionFormat::Xz => XZ_EXTENSION,
        CompressionFormat::Lzma => LZMA_EXTENSION,
        CompressionFormat::Auto => XZ_EXTENSION,  // Default to XZ for auto
    }
}

/// Generates an output filename based on input path and operation mode
pub fn generate_output_filename(input: &Path, config: &CliConfig) -> Result<PathBuf> {
    match config.mode {
        OperationMode::Compress => {
            let mut output = input.to_path_buf();
            let ext = get_extension(config);
            
            // Preserve original extension and add .xz/.lzma
            if let Some(old_ext) = input.extension().and_then(OsStr::to_str) {
                if !old_ext.is_empty() {
                    let new_ext = format!("{}.{}", old_ext, ext);
                    output.set_extension(new_ext);
                    return Ok(output);
                }
            }
            output.set_extension(ext);
            Ok(output)
        }
        OperationMode::Decompress | OperationMode::Cat => {
            if !has_compression_extension(input) {
                return Err(Error::InvalidExtension {
                    path: input.to_path_buf(),
                });
            }
            
            let file_name = input.file_name()
                .ok_or_else(|| Error::InvalidOutputFilename { 
                    path: input.to_path_buf() 
                })?;
            
            let file_name_str = file_name.to_string_lossy();
            
            // Remove .xz or .lzma suffix (case insensitive)
            let stem = file_name_str
                .trim_end_matches(".xz")
                .trim_end_matches(".XZ")
                .trim_end_matches(".lzma")
                .trim_end_matches(".LZMA");
            
            // If nothing remains after removing extension, use original
            let final_stem = if stem.is_empty() { &file_name_str } else { stem };
            
            let parent = input.parent().unwrap_or_else(|| Path::new("."));
            Ok(parent.join(final_stem))
        }
        OperationMode::Test => Ok(PathBuf::new()),
    }
}

/// Opens an input reader for the given path, or stdin if path is empty
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

/// Opens an output writer for the given path or stdout
pub fn open_output(path: Option<&Path>, config: &CliConfig) -> Result<Box<dyn Write>> {
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
