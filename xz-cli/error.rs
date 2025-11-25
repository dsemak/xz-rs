//! Error types for XZ CLI operations.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Main error type for XZ CLI operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to open input file
    #[error("{path}: {source}")]
    OpenInput {
        /// Path to the input file
        path: String,
        /// Underlying I/O error
        #[source]
        source: io::Error,
    },

    /// Failed to create output file
    #[error("{}: {source}", path.display())]
    CreateOutput {
        /// Path to the output file
        path: PathBuf,
        /// Underlying I/O error
        #[source]
        source: io::Error,
    },

    /// Output file already exists
    #[error("{}: Output file already exists", path.display())]
    OutputExists {
        /// Path to the existing file
        path: PathBuf,
    },

    /// Input file lacks recognized compression extension
    #[error("{}: Filename has an unknown suffix, skipping", path.display())]
    InvalidExtension {
        /// Path to the input file
        path: PathBuf,
    },

    /// Cannot determine output filename
    #[error("{}: Cannot determine output filename", path.display())]
    InvalidOutputFilename {
        /// Path to the input file
        path: PathBuf,
    },

    /// Compression operation failed
    #[error("{path}: Compressed data is corrupt")]
    Compression {
        /// Path to the file being compressed
        path: String,
        /// Error message from liblzma
        message: String,
    },

    /// Decompression operation failed
    #[error("{path}: Compressed data is corrupt")]
    Decompression {
        /// Path to the file being decompressed
        path: String,
        /// Error message from liblzma
        message: String,
    },

    /// Invalid compression level
    #[error("Unsupported preset: {level}")]
    InvalidCompressionLevel {
        /// The invalid level value
        level: u32,
    },

    /// Thread count too large
    #[error("The number of threads must not exceed {}", u32::MAX)]
    InvalidThreadCount {
        /// The invalid thread count
        count: usize,
    },

    /// Failed to remove input file
    #[error("{path}: Cannot remove: {source}")]
    RemoveFile {
        /// Path to the file
        path: String,
        /// Underlying I/O error
        #[source]
        source: io::Error,
    },

    /// Invalid memory limit format
    #[error("Invalid memory limit: {0}")]
    InvalidMemoryLimit(String),

    /// Read error
    #[error("{path}: Read error: {source}")]
    ReadError {
        /// Path to the file
        path: String,
        /// Underlying I/O error
        #[source]
        source: io::Error,
    },

    /// Write error
    #[error("{path}: Write error: {source}")]
    WriteError {
        /// Path to the file
        path: String,
        /// Underlying I/O error
        #[source]
        source: io::Error,
    },

    /// Empty filename
    #[error("Empty filename, skipping")]
    EmptyFilename,

    /// Memory limit exceeded during decompression
    #[error("{path}: Memory usage limit reached")]
    MemoryLimitExceeded {
        /// Path to the file
        path: String,
    },
}

/// Specialized `Result` type for XZ CLI operations.
pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for io::Error {
    fn from(err: Error) -> Self {
        match &err {
            Error::OutputExists { .. } => io::Error::new(io::ErrorKind::AlreadyExists, err),
            Error::InvalidExtension { .. }
            | Error::InvalidOutputFilename { .. }
            | Error::InvalidCompressionLevel { .. }
            | Error::InvalidThreadCount { .. }
            | Error::InvalidMemoryLimit(_)
            | Error::EmptyFilename => io::Error::new(io::ErrorKind::InvalidInput, err),
            Error::Decompression { .. } | Error::Compression { .. } => {
                io::Error::new(io::ErrorKind::InvalidData, err)
            }
            Error::OpenInput { source, .. }
            | Error::CreateOutput { source, .. }
            | Error::RemoveFile { source, .. }
            | Error::ReadError { source, .. }
            | Error::WriteError { source, .. } => {
                // Preserve the original error kind
                io::Error::new(source.kind(), err)
            }
            Error::MemoryLimitExceeded { .. } => io::Error::new(io::ErrorKind::OutOfMemory, err),
        }
    }
}
