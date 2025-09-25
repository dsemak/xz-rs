//! Common CLI utilities and shared functionality for XZ command-line tools.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
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
    pub level: Option<u8>,
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
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            ext.eq_ignore_ascii_case(XZ_EXTENSION) || ext.eq_ignore_ascii_case(LZMA_EXTENSION)
        })
        .unwrap_or(false)
}

/// Generates output filename based on input filename and operation mode
pub fn generate_output_filename(input: &Path, mode: OperationMode) -> Result<PathBuf> {
    match mode {
        OperationMode::Compress => {
            let mut output = input.to_path_buf();
            let current_ext = input.extension().and_then(OsStr::to_str).unwrap_or("");
            if current_ext.is_empty() {
                output.set_extension(XZ_EXTENSION);
            } else {
                output.set_extension(format!("{}.{}", current_ext, XZ_EXTENSION));
            }
            Ok(output)
        }
        OperationMode::Decompress | OperationMode::Cat => {
            if !has_compression_extension(input) {
                anyhow::bail!(
                    "Input file '{}' does not have a recognized compression extension",
                    input.display()
                );
            }

            let stem = input
                .file_stem()
                .context("Cannot determine output filename")?;

            let parent = input.parent().unwrap_or_else(|| Path::new("."));
            Ok(parent.join(stem))
        }
        OperationMode::Test => Ok(PathBuf::new()), // Not used for test mode
    }
}

/// Opens input reader, handling stdin if path is "-"
pub fn open_input(path: &str) -> Result<Box<dyn Read>> {
    if path == "-" {
        Ok(Box::new(io::stdin()))
    } else {
        let file =
            File::open(path).with_context(|| format!("Failed to open input file '{}'", path))?;
        Ok(Box::new(BufReader::new(file)))
    }
}

/// Opens output writer, handling stdout if path is "-" or config.stdout is true
pub fn open_output(path: Option<&Path>, config: &CliConfig) -> Result<Box<dyn Write>> {
    if config.stdout || path.map(|p| p.as_os_str()) == Some(OsStr::new("-")) {
        Ok(Box::new(io::stdout()))
    } else if let Some(path) = path {
        // Check if output file exists and we're not forcing overwrite
        if path.exists() && !config.force {
            anyhow::bail!(
                "Output file '{}' already exists. Use --force to overwrite.",
                path.display()
            );
        }

        let file = File::create(path)
            .with_context(|| format!("Failed to create output file '{}'", path.display()))?;
        Ok(Box::new(BufWriter::new(file)))
    } else {
        Ok(Box::new(io::stdout()))
    }
}

/// Performs compression operation
pub fn compress_file(
    mut input: Box<dyn Read>,
    mut output: Box<dyn Write>,
    config: &CliConfig,
) -> Result<()> {
    let mut options = CompressionOptions::default();

    if let Some(level) = config.level {
        use xz_core::options::Compression;
        let compression_level = match level {
            0 => Compression::Level0,
            1 => Compression::Level1,
            2 => Compression::Level2,
            3 => Compression::Level3,
            4 => Compression::Level4,
            5 => Compression::Level5,
            6 => Compression::Level6,
            7 => Compression::Level7,
            8 => Compression::Level8,
            9 => Compression::Level9,
            _ => anyhow::bail!("Invalid compression level: {}. Must be 0-9.", level),
        };
        options = options.with_level(compression_level);
    }

    if let Some(threads) = config.threads {
        use xz_core::Threading;
        let thread_count = u32::try_from(threads)
            .map_err(|_| anyhow::anyhow!("Thread count {} is too large", threads))?;
        options = options.with_threads(Threading::Exact(thread_count));
    }

    let summary = match compress(&mut input, &mut output, &options) {
        Ok(summary) => summary,
        Err(e) => {
            eprintln!("Compression failed: {}", e);
            return Err(e.into());
        }
    };

    if config.verbose {
        eprintln!(
            "Compressed {} bytes to {} bytes ({:.1}% ratio)",
            summary.bytes_read,
            summary.bytes_written,
            (summary.bytes_written as f64 / summary.bytes_read as f64) * 100.0
        );
    }

    Ok(())
}

/// Performs decompression operation
pub fn decompress_file(
    mut input: Box<dyn Read>,
    mut output: Box<dyn Write>,
    config: &CliConfig,
) -> Result<()> {
    let mut options = DecompressionOptions::default();

    if let Some(threads) = config.threads {
        use xz_core::Threading;
        let thread_count = u32::try_from(threads)
            .map_err(|_| anyhow::anyhow!("Thread count {} is too large", threads))?;
        options = options.with_threads(Threading::Exact(thread_count));
    }

    if let Some(memory_limit) = config.memory_limit {
        use std::num::NonZeroU64;
        if let Some(limit) = NonZeroU64::new(memory_limit) {
            options = options.with_memlimit(limit);
        }
    }

    let summary = decompress(&mut input, &mut output, &options).context("Decompression failed")?;

    if config.verbose {
        eprintln!(
            "Decompressed {} bytes to {} bytes",
            summary.bytes_read, summary.bytes_written
        );
    }

    Ok(())
}

/// Removes input file if configured to do so
pub fn cleanup_input_file(input_path: &str, config: &CliConfig) -> Result<()> {
    if !config.keep && input_path != "-" && !config.stdout {
        std::fs::remove_file(input_path)
            .with_context(|| format!("Failed to remove input file '{}'", input_path))?;

        if config.verbose {
            eprintln!("Removed input file: {}", input_path);
        }
    }
    Ok(())
}

/// Processes a single file according to the configuration
pub fn process_file(input_path: &str, config: &CliConfig) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
