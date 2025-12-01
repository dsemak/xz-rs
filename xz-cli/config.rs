//! Configuration types and constants for XZ CLI operations.

use xz_core::config::DecodeMode;
use xz_core::options::IntegrityCheck;

/// Default buffer size for file I/O operations
pub const DEFAULT_BUFFER_SIZE: usize = 512 * 1024;

/// File extension for XZ compressed files
pub const XZ_EXTENSION: &str = "xz";

/// File extension for LZMA compressed files
pub const LZMA_EXTENSION: &str = "lzma";

/// Represents different modes of operation for CLI utilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    /// Decompress and output to stdout (like cat)
    Cat,
    /// Compress input data
    Compress,
    /// Decompress input data
    Decompress,
    /// List information about compressed files
    List,
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
    /// Quiet mode (suppress warnings)
    pub quiet: bool,
    /// Compression level (0-9)
    pub level: Option<u32>,
    /// Number of threads to use
    pub threads: Option<usize>,
    /// Memory limit for decompression
    pub memory_limit: Option<u64>,
    /// Use extreme compression
    pub extreme: bool,
    /// File format to use
    pub format: DecodeMode,
    /// Integrity check type
    pub check: IntegrityCheck,
    /// Machine-readable output
    pub robot: bool,
    /// Custom suffix for compressed files
    pub suffix: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            mode: OperationMode::Compress,
            force: false,
            keep: false,
            stdout: false,
            verbose: false,
            quiet: false,
            level: None,
            threads: None,
            memory_limit: None,
            extreme: false,
            format: DecodeMode::Auto,
            check: IntegrityCheck::Crc64,
            robot: false,
            suffix: None,
        }
    }
}
