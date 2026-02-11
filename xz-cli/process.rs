//! High-level file processing and CLI orchestration.

use std::io;
use std::path::PathBuf;

use crate::config::{CliConfig, OperationMode};
use crate::error::{DiagnosticCause, Error, ExitStatus, IoErrorNoCode, Report, Result};
use crate::format::list::{print_list_totals, ListOutputContext, ListSummary};
use crate::io::{
    generate_output_filename, open_input, open_output, open_output_file, SparseFileWriter,
};
use crate::operations::{compress_file, decompress_file, list_file, list_file_with_context};

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
    if config.mode == OperationMode::Test || config.mode == OperationMode::List {
        return Ok(());
    }

    let is_stdin = input_path.is_empty() || input_path == "-";

    if !config.keep && !is_stdin && !config.stdout {
        std::fs::remove_file(input_path).map_err(|source| {
            DiagnosticCause::from(Error::RemoveFile {
                source: IoErrorNoCode::new(source),
            })
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
/// Returns an error in these cases:
///
/// - Input file cannot be opened or read
/// - Output filename generation fails (e.g., decompressing file without valid extension)
/// - Output file creation fails (permissions, disk space, etc.)
/// - Output file exists and `force` flag is not set
/// - Compression/decompression operation fails
/// - Input file removal fails (when cleanup is enabled)
pub fn process_file(input_path: &str, config: &CliConfig) -> Result<()> {
    let is_stdin = input_path.is_empty() || input_path == "-";

    // Use empty PathBuf for stdin, otherwise use the provided path
    let input_path_buf = if is_stdin {
        PathBuf::new()
    } else {
        PathBuf::from(input_path)
    };

    let input = open_input(input_path)?;

    // Determine output path
    let output_path = if is_stdin
        || config.stdout
        || config.mode == OperationMode::Cat
        || config.mode == OperationMode::Test
    {
        None
    } else {
        let default_extension = match (config.mode, config.format) {
            (OperationMode::Compress, xz_core::config::DecodeMode::Lzma) => {
                crate::config::LZMA_EXTENSION
            }
            _ => crate::config::XZ_EXTENSION,
        };
        Some(generate_output_filename(
            &input_path_buf,
            config.mode,
            config.suffix.as_deref(),
            default_extension,
            config.force,
        )?)
    };

    // Open output
    let output: Box<dyn io::Write> = match (
        config.mode,
        config.sparse,
        config.stdout,
        output_path.as_deref(),
    ) {
        (OperationMode::Decompress, true, false, Some(path)) => {
            // When decompressing to a file, attempt to create sparse output by seeking over
            // long zero runs
            let file = open_output_file(path, config)?;
            Box::new(SparseFileWriter::new(file))
        }
        _ => open_output(output_path.as_deref(), config)?,
    };

    // Process based on mode
    match config.mode {
        OperationMode::Compress => {
            compress_file(input, output, config)?;
        }
        OperationMode::Decompress | OperationMode::Cat => {
            match decompress_file(input, output, config) {
                Ok(()) => (),
                Err(DiagnosticCause::Warning(
                    w @ crate::error::Warning::UnsupportedCheck { .. },
                )) => {
                    cleanup_input_file(input_path, config)?;
                    return Err(DiagnosticCause::Warning(w));
                }
                Err(other) => return Err(other),
            }
        }
        OperationMode::Test => {
            // In test mode, decompress but discard output
            decompress_file(input, io::sink(), config)?;

            if config.verbose || config.robot {
                if config.robot {
                    println!("OK {input_path}");
                } else {
                    eprintln!("Test successful: {input_path}");
                }
            }
        }
        OperationMode::List => {
            list_file(input_path, config)?;
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
pub fn parse_memory_limit(s: &str) -> Result<u64> {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    let s = s.trim();
    if s.is_empty() {
        return Err(DiagnosticCause::from(Error::InvalidMemoryLimit(
            "Empty memory limit".to_string(),
        )));
    }

    let (number_part, multiplier) = if let Some(last_char) = s.chars().last() {
        match last_char.to_ascii_uppercase() {
            'K' => (&s[..s.len() - 1], KB),
            'M' => (&s[..s.len() - 1], MB),
            'G' => (&s[..s.len() - 1], GB),
            _ if last_char.is_ascii_digit() => (s, 1),
            _ => {
                return Err(DiagnosticCause::from(Error::InvalidMemoryLimit(format!(
                    "Invalid memory limit suffix: {last_char}"
                ))))
            }
        }
    } else {
        (s, 1)
    };

    let number: u64 = number_part.parse().map_err(|_| {
        DiagnosticCause::from(Error::InvalidMemoryLimit(format!(
            "Invalid number: {number_part}"
        )))
    })?;

    number.checked_mul(multiplier).ok_or_else(|| {
        DiagnosticCause::from(Error::InvalidMemoryLimit(
            "Memory limit too large (overflow)".to_string(),
        ))
    })
}

/// Processes multiple files in list mode, accumulating totals and handling multi-file output.
///
/// # Parameters
///
/// * `files` - Slice of input file paths to list
/// * `config` - CLI configuration
/// * `program` - Program name for error messages
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if any file operation fails.
/// Gracefully handles `BrokenPipe` errors by returning `Ok(())`.
fn process_list_files(files: &[String], config: &CliConfig, program: &str) -> Report {
    let mut report = Report::default();
    let total = files.len();
    let mut header_printed = false;
    let mut totals = ListSummary::default();

    for (idx, file) in files.iter().enumerate() {
        let ctx = ListOutputContext {
            file_index: idx + 1,
            file_count: total,
            print_header: !config.robot && !config.verbose && !header_printed,
        };
        header_printed |= ctx.print_header;

        match list_file_with_context(file, config, ctx) {
            Ok(summary) => {
                totals.stream_count += summary.stream_count;
                totals.block_count += summary.block_count;
                totals.compressed += summary.compressed;
                totals.uncompressed += summary.uncompressed;
                totals.checks_mask |= summary.checks_mask;
            }
            Err(err) => {
                // Handle broken pipe gracefully (e.g., when piping to `head`).
                if is_broken_pipe(&err) {
                    return report;
                }
                report.record(err, program, Some(file));
            }
        }
    }

    // Print summary line for multiple files (non-verbose, non-robot mode)
    if total > 1 && !config.robot && !config.verbose {
        if let Err(err) = print_list_totals(totals, total) {
            if is_broken_pipe(&err) {
                return report;
            }
            report.record(err, program, None);
        }
    }

    report
}

/// Processes multiple files sequentially in non-list modes.
///
/// # Parameters
///
/// * `files` - Slice of input file paths to process
/// * `config` - CLI configuration
/// * `program` - Program name for error messages
///
/// # Returns
///
/// Returns `Ok(())` if all files were processed successfully.
fn process_files(files: &[String], config: &CliConfig, program: &str) -> Report {
    let mut report = Report::default();
    for file in files {
        match process_file(file, config) {
            Ok(()) => {}
            Err(err) => {
                if is_broken_pipe(&err) {
                    return report;
                }
                report.record(err, program, Some(file));
            }
        }
    }
    report
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
/// Returns a [`Report`] containing the aggregated exit status and all diagnostics.
///
/// # Errors
///
/// This function does not fail fast. It continues processing remaining files
/// after per-file errors and aggregates the exit code like upstream `xz`.
pub fn run_cli(files: &[String], config: &CliConfig, program: &str) -> Report {
    let mut report = Report::default();

    if config.mode == OperationMode::List && files.is_empty() {
        report.record(
            DiagnosticCause::from(Error::ListModeStdinUnsupported),
            program,
            None,
        );
        report.status = ExitStatus::Error;
        return report;
    }

    if files.is_empty() {
        match process_file("", config) {
            Ok(()) => {}
            Err(err) => {
                if !is_broken_pipe(&err) {
                    report.record(err, program, None);
                }
            }
        }
    } else if config.mode == OperationMode::List {
        report = process_list_files(files, config, program);
    } else {
        report = process_files(files, config, program);
    }

    report
}

/// Returns `true` if the diagnostic cause is a `BrokenPipe` write error.
fn is_broken_pipe(err: &DiagnosticCause) -> bool {
    match err.as_error() {
        Some(crate::error::Error::WriteOutput { source }) => {
            source.kind() == io::ErrorKind::BrokenPipe
        }
        _ => false,
    }
}
