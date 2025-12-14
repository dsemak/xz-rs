//! Error types for XZ CLI operations.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// A structured CLI error that preserves the underlying failure.
///
/// This is used to implement `-q/-qq` output suppression while keeping rich
/// error context (program name and input file).
#[derive(Debug)]
pub struct InvocationError {
    /// Program name to prefix in error output (e.g. "xz", "unxz").
    pub program: String,
    /// Input file path, or `None` for stdin.
    pub file: Option<String>,
    /// Underlying error produced by processing.
    pub source: CliError,
}

impl std::fmt::Display for InvocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.file.as_deref() {
            Some(file) => write!(f, "{}: {}: {}", self.program, file, self.source),
            None => write!(f, "{}: (stdin): {}", self.program, self.source),
        }
    }
}

impl std::error::Error for InvocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Formats an error message for stderr, respecting `-q/-qq`.
///
/// # Parameters
///
/// - `program`: Program name prefix to use in error output (e.g. `"xz"`).
/// - `quiet`: Quiet level (as counted by `-q` occurrences).
/// - `err`: The I/O error returned by the CLI runner.
///
/// # Returns
///
/// Returns `None` when the message should be suppressed by `quiet`,
/// otherwise returns a formatted single-line message suitable for stderr.
pub fn format_error_for_stderr(program: &str, quiet: u8, err: &io::Error) -> Option<String> {
    if quiet >= 2 {
        return None;
    }

    let run_err = err
        .get_ref()
        .and_then(|e| e.downcast_ref::<InvocationError>());

    if quiet >= 1 && run_err.is_some_and(|e| e.source.as_warning().is_some()) {
        return None;
    }

    if let Some(run_err) = run_err {
        return Some(run_err.to_string());
    }

    Some(format!("{program}: {err}"))
}

/// Warning conditions for XZ CLI operations.
///
/// These are non-fatal conditions that upstream `xz` typically reports as a
/// warning/notice.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Warning {
    /// Input file lacks recognized compression extension
    #[error("{}: Filename has an unknown suffix, skipping", path.display())]
    InvalidExtension {
        /// Path to the input file
        path: PathBuf,
    },

    /// Input file already has the target suffix
    #[error("{}: Already has `{}` suffix, skipping", path.display(), suffix)]
    AlreadyHasSuffix {
        /// Path to the input file
        path: PathBuf,
        /// The suffix that already exists
        suffix: String,
    },
}

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

    /// Failed to extract file information
    #[error("{path}: File format not recognized")]
    FileInfoExtraction {
        /// Path to the file
        path: String,
        /// Error message
        message: String,
    },
}

/// Specialized `Result` type for XZ CLI operations.
pub type Result<T> = std::result::Result<T, CliError>;

/// This represents both "real" failures and warning/notice conditions.
#[derive(Debug, Error)]
pub enum CliError {
    /// Warning/notice condition.
    #[error(transparent)]
    Warning(#[from] Warning),

    /// Real failure condition.
    #[error(transparent)]
    Error(#[from] Error),
}

impl CliError {
    /// Returns a reference to the warning if this error represents a warning/notice.
    pub fn as_warning(&self) -> Option<&Warning> {
        match self {
            CliError::Warning(w) => Some(w),
            CliError::Error(_) => None,
        }
    }

    /// Returns a reference to the underlying "real" error, if any.
    pub fn as_error(&self) -> Option<&Error> {
        match self {
            CliError::Warning(_) => None,
            CliError::Error(e) => Some(e),
        }
    }
}

impl From<CliError> for io::Error {
    fn from(err: CliError) -> Self {
        match &err {
            CliError::Warning(_) => io::Error::new(io::ErrorKind::InvalidInput, err),
            CliError::Error(source) => match source {
                Error::OutputExists { .. } => io::Error::new(io::ErrorKind::AlreadyExists, err),
                Error::InvalidOutputFilename { .. }
                | Error::InvalidCompressionLevel { .. }
                | Error::InvalidThreadCount { .. }
                | Error::InvalidMemoryLimit(_) => io::Error::new(io::ErrorKind::InvalidInput, err),
                Error::Compression { .. }
                | Error::Decompression { .. }
                | Error::FileInfoExtraction { .. } => {
                    io::Error::new(io::ErrorKind::InvalidData, err)
                }
                Error::OpenInput { source, .. }
                | Error::CreateOutput { source, .. }
                | Error::RemoveFile { source, .. } => {
                    // Preserve the original error kind
                    io::Error::new(source.kind(), err)
                }
            },
        }
    }
}
