//! Common CLI utilities and shared functionality for XZ command-line tools.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::Context;
use xz_core::{
    options::{CompressionOptions, DecompressionOptions},
    pipeline::{compress, decompress},
};

/// Default buffer size for file I/O operations
pub const DEFAULT_BUFFER_SIZE: usize = 64 * 1024;

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

/// Determines if a file has an XZ or LZMA extension
pub fn has_compression_extension(path: &Path) -> bool {
    match path.extension().and_then(OsStr::to_str) {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            ext == XZ_EXTENSION || ext == LZMA_EXTENSION
        }
        None => false,
    }
}

/// Generates an output filename based on the input filename and operation mode.
///
/// # Parameters
///
/// * `input` - The input file path.
/// * `mode` - The operation mode (Compress, Decompress, Cat, or Test).
///
/// # Errors
///
/// Returns an error if the output filename cannot be determined (e.g., missing file stem on decompress).
pub fn generate_output_filename(input: &Path, mode: OperationMode) -> io::Result<PathBuf> {
    match mode {
        OperationMode::Compress => {
            let mut output = input.to_path_buf();
            match input.extension().and_then(OsStr::to_str) {
                Some(ext) if !ext.is_empty() => {
                    // Add .xz after the existing extension, e.g. file.txt -> file.txt.xz
                    output.set_extension(format!("{ext}.{XZ_EXTENSION}"));
                }
                _ => {
                    // No extension, just add .xz
                    output.set_extension(XZ_EXTENSION);
                }
            }
            Ok(output)
        }
        OperationMode::Decompress | OperationMode::Cat => {
            // Check for a valid compression extension
            if !has_compression_extension(input) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Input file '{}' does not have a recognized compression extension",
                        input.display()
                    ),
                ));
            }
            // Try to get the file stem (filename without last extension)
            let stem = input.file_stem().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Cannot determine output filename for '{}'", input.display()),
                )
            })?;

            // Use the parent directory or current directory if none
            let parent = input.parent().unwrap_or_else(|| Path::new("."));
            Ok(parent.join(stem))
        }
        // Not used for test mode
        OperationMode::Test => Ok(PathBuf::new()),
    }
}

/// Opens an input reader for the given path, or stdin if path is "-".
///
/// # Parameters
///
/// * `path` - Path to the input file, or "-" for stdin.
///
/// # Errors
///
/// Returns an error if the file cannot be opened.
pub fn open_input(path: &str) -> io::Result<Box<dyn io::Read>> {
    match path {
        "-" => {
            // stdin is always available, so this is safe
            Ok(Box::new(io::stdin()))
        }
        _ => {
            // Try to open the file and wrap it in a buffered reader
            let file = File::open(path).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to open input file '{}'", path),
                )
            })?;
            Ok(Box::new(io::BufReader::new(file)))
        }
    }
}

/// Opens an output writer for the given path, or stdout if path is "-" or config.stdout is true.
///
/// # Parameters
///
/// * `path` - Optional path to the output file. If None or "-", writes to stdout.
/// * `config` - CLI configuration, used to determine if stdout should be used.
///
/// # Errors
///
/// Returns an error if the file cannot be created or if the file exists and force is not set.
pub fn open_output(path: Option<&Path>, config: &CliConfig) -> io::Result<Box<dyn io::Write>> {
    // Determine if we should write to stdout
    let use_stdout = config.stdout || path.map(|p| p == Path::new("-")).unwrap_or(true);

    if use_stdout {
        // stdout is always available, so this is safe
        Ok(Box::new(io::stdout()))
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
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to create output file '{}'", path.display()),
            )
        })?;

        Ok(Box::new(io::BufWriter::new(file)))
    } else {
        // Fallback to stdout if no path is provided
        Ok(Box::new(io::stdout()))
    }
}

/// Compresses input data and writes the result to output.
///
/// # Parameters
///
/// * `input` - Input reader.
/// * `output` - Output writer.
/// * `config` - CLI configuration.
///
/// # Errors
///
/// Returns an error if compression fails or if invalid options are provided.
pub fn compress_file(
    mut input: Box<dyn io::Read>,
    mut output: Box<dyn io::Write>,
    config: &CliConfig,
) -> io::Result<()> {
    let mut options = CompressionOptions::default();

    // Set compression level if specified
    if let Some(level) = config.level {
        let compression_level = xz_core::options::Compression::try_from(level)?;
        options = options.with_level(compression_level);
    }

    // Set thread count if specified
    if let Some(threads) = config.threads {
        let thread_count = u32::try_from(threads).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Thread count {} is too large", threads),
            )
        })?;
        options = options.with_threads(xz_core::Threading::Exact(thread_count));
    }

    // Perform compression and handle errors
    let summary = compress(&mut input, &mut output, &options)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Compression failed: {e}")))?;

    // Print verbose output if enabled
    if config.verbose {
        let ratio = if summary.bytes_read > 0 {
            (summary.bytes_written as f64 / summary.bytes_read as f64) * 100.0
        } else {
            0.0
        };

        eprintln!(
            "Compressed {} bytes to {} bytes ({:.1}% ratio)",
            summary.bytes_read, summary.bytes_written, ratio
        );
    }

    Ok(())
}

/// Decompresses input data and writes the result to output.
///
/// * `input` - Input reader.
/// * `output` - Output writer.
/// * `config` - CLI configuration.
///
/// # Errors
///
/// Returns an error if decompression fails or if invalid options are provided.
pub fn decompress_file(
    mut input: Box<dyn io::Read>,
    mut output: Box<dyn io::Write>,
    config: &CliConfig,
) -> io::Result<()> {
    let mut options = DecompressionOptions::default();

    // Set thread count if specified
    if let Some(threads) = config.threads {
        let thread_count = u32::try_from(threads).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Thread count {} is too large", threads),
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
    let summary = decompress(&mut input, &mut output, &options)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Decompression failed: {e}")))?;

    // Print verbose output if enabled
    if config.verbose {
        let ratio = if summary.bytes_read > 0 {
            (summary.bytes_written as f64 / summary.bytes_read as f64) * 100.0
        } else {
            0.0
        };

        eprintln!(
            "Decompressed {} bytes to {} bytes ({:.1}% expansion)",
            summary.bytes_read, summary.bytes_written, ratio
        );
    }

    Ok(())
}

/// Removes input file if configured to do so
pub fn cleanup_input_file(input_path: &str, config: &CliConfig) -> io::Result<()> {
    if !config.keep && input_path != "-" && !config.stdout {
        std::fs::remove_file(input_path).map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("Failed to remove input file '{}': {err}", input_path),
            )
        })?;

        if config.verbose {
            eprintln!("Removed input file: {}", input_path);
        }
    }
    Ok(())
}

/// Processes a single file according to the configuration
pub fn process_file(input_path: &str, config: &CliConfig) -> io::Result<()> {
    let input_path_buf = if input_path == "-" {
        PathBuf::from("-")
    } else {
        PathBuf::from(input_path)
    };

    // Open input
    let input = open_input(input_path)?;

    // Determine output path
    let output_path = if config.stdout || config.mode == OperationMode::Cat {
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
            // For test mode, decompress but discard output
            let null_output = Box::new(io::sink());
            decompress_file(input, null_output, config)?;
            if config.verbose {
                eprintln!("Test successful: {}", input_path);
            }
        }
    }

    // Cleanup input file if needed
    cleanup_input_file(input_path, config)?;

    Ok(())
}

/// Parse memory limit with optional suffix (K, M, G)
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

/// Run a CLI command over the provided inputs, forwarding contextual errors.
pub fn run_cli(files: &[String], config: &CliConfig, program: &str) -> anyhow::Result<()> {
    if files.is_empty() {
        process_file("-", config).with_context(|| format!("{program}: -"))?;
    } else {
        for file in files {
            process_file(file, config).with_context(|| format!("{program}: {file}"))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Test memory limit parsing
    #[test]
    fn test_parse_memory_limit() {
        assert_eq!(parse_memory_limit("1024").unwrap(), 1024);
        assert_eq!(parse_memory_limit("1K").unwrap(), 1024);
        assert_eq!(parse_memory_limit("1M").unwrap(), 1024 * 1024);
        assert_eq!(parse_memory_limit("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_limit("512k").unwrap(), 512 * 1024);

        assert!(parse_memory_limit("").is_err());
        assert!(parse_memory_limit("invalid").is_err());
        assert!(parse_memory_limit("1X").is_err());
    }

    #[test]
    fn test_has_compression_extension() {
        assert!(has_compression_extension(Path::new("file.xz")));
        assert!(has_compression_extension(Path::new("file.lzma")));
        assert!(has_compression_extension(Path::new("FILE.XZ")));
        assert!(has_compression_extension(Path::new("FILE.LZMA")));
        assert!(!has_compression_extension(Path::new("file.txt")));
        assert!(!has_compression_extension(Path::new("file")));
    }

    #[test]
    fn test_generate_output_filename_compress() {
        let input = Path::new("test.txt");
        let output = generate_output_filename(input, OperationMode::Compress).unwrap();
        assert_eq!(output, PathBuf::from("test.txt.xz"));

        let input = Path::new("test");
        let output = generate_output_filename(input, OperationMode::Compress).unwrap();
        assert_eq!(output, PathBuf::from("test.xz"));
    }

    #[test]
    fn test_generate_output_filename_decompress() {
        let input = Path::new("test.txt.xz");
        let output = generate_output_filename(input, OperationMode::Decompress).unwrap();
        assert_eq!(output, PathBuf::from("test.txt"));

        let input = Path::new("test.lzma");
        let output = generate_output_filename(input, OperationMode::Decompress).unwrap();
        assert_eq!(output, PathBuf::from("test"));
    }

    #[test]
    fn test_generate_output_filename_decompress_invalid() {
        let input = Path::new("test.txt");
        let result = generate_output_filename(input, OperationMode::Decompress);
        assert!(result.is_err());
    }
}
