//! High-level file processing and CLI orchestration.

use std::io;
use std::path::PathBuf;

use crate::config::{CliConfig, OperationMode};
use crate::error::{Error, Result};
use crate::io::{generate_output_filename, open_input, open_output};
use crate::operations::{compress_file, decompress_file};

/// Removes the input file after successful processing.
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
            Some(generate_output_filename(&input_path_buf, config)?)
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
