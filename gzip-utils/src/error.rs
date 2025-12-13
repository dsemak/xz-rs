//! Error types for compression CLI operations.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Main error type for compression CLI operations.
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
    #[error("{path}: Compression failed: {message}")]
    Compression {
        /// Path to the file being compressed
        path: String,
        /// Error message from liblzma
        message: String,
    },

    /// Decompression operation failed
    #[error("{path}: Decompression failed: {message}")]
    Decompression {
        /// Path to the file being decompressed
        path: String,
        /// Error message from liblzma
        message: String,
    },

    /// Invalid compression level
    #[error("Unsupported compression level: {level} (must be 0-9)")]
    InvalidCompressionLevel {
        /// The invalid level value
        level: u32,
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

    /// General I/O error
     #[error("I/O error: {source}")]
    Io {
        /// Underlying I/O error
        #[source]
        source: io::Error,
    },

    /// Generic error with message
    #[error("{0}")]
    Generic(String),
}

/// Specialized `Result` type for gzip CLI operations.
pub type Result<T> = std::result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Error::Io { source }
    }
}


impl From<Error> for io::Error {
    fn from(err: Error) -> Self {
        match &err {
            Error::OutputExists { .. } => io::Error::new(io::ErrorKind::AlreadyExists, err),
            Error::InvalidExtension { .. }
            | Error::InvalidOutputFilename { .. }
            | Error::InvalidCompressionLevel { .. }

            | Error::Generic(_) => io::Error::new(io::ErrorKind::InvalidInput, err),
            Error::Decompression { .. } | Error::Compression { .. } => {
                io::Error::new(io::ErrorKind::InvalidData, err)
            }
            Error::OpenInput { source, .. }
            | Error::CreateOutput { source, .. }
            | Error::RemoveFile { source, .. } 
            | Error::Io { source } => {
                // Preserve the original error kind
                io::Error::new(source.kind(), err)
            }
        }
    }
}
