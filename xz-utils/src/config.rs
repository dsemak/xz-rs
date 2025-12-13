//! Configuration types and constants for XZ/LZMA CLI operations.

/// Default buffer size for file I/O operations
pub const DEFAULT_BUFFER_SIZE: usize = 512 * 1024;

/// File extension for XZ compressed files
pub const XZ_EXTENSION: &str = "xz";

/// File extension for LZMA compressed files  
pub const LZMA_EXTENSION: &str = "lzma";

/// Compression format to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionFormat {
    /// Auto-detect format (based on file extension or magic bytes)
    Auto,
    /// Use XZ format (LZMA2, recommended)
    Xz,
    /// Use legacy LZMA format
    Lzma,
}

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
    pub level: Option<u32>,
    /// Compression format to use
    pub format: CompressionFormat,
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
            format: CompressionFormat::Xz,  // Default to XZ format
        }
    }
}
