//! Error types for XZ CLI operations.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Formats `std::io::Error` similar to `strerror(3)` output, without the trailing
/// `"(os error N)"` suffix that Rust includes by default.
#[derive(Debug)]
pub struct IoErrorNoCode {
    inner: io::Error,
}

impl IoErrorNoCode {
    /// Creates a new wrapper around an I/O error.
    pub fn new(inner: io::Error) -> Self {
        Self { inner }
    }

    /// Returns the underlying I/O error kind.
    pub fn kind(&self) -> io::ErrorKind {
        self.inner.kind()
    }
}

impl std::fmt::Display for IoErrorNoCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut msg = self.inner.to_string();

        // Rust formats OS errors as "X (os error N)" which doesn't match upstream xz tools.
        // Strip that suffix when present.
        if msg.ends_with(')') {
            if let Some(idx) = msg.rfind(" (os error ") {
                msg.truncate(idx);
            }
        }

        write!(f, "{msg}")
    }
}

impl std::error::Error for IoErrorNoCode {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner)
    }
}

/// Severity of a CLI diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Non-fatal condition (upstream usually prints as a warning/notice).
    Warning,
    /// Fatal condition.
    Error,
}

/// Aggregated exit status for processing multiple input files.
///
/// This follows upstream `xz` semantics:
/// - `0`: success (no warnings/errors)
/// - `1`: at least one real error occurred
/// - `2`: only warnings occurred (no real errors)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExitStatus {
    /// No warnings or errors.
    #[default]
    Ok,
    /// One or more warnings, but no real errors.
    Warning,
    /// One or more real errors (takes precedence over warnings).
    Error,
}

impl ExitStatus {
    /// Returns the numeric exit code corresponding to this status.
    pub const fn code(self) -> i32 {
        match self {
            ExitStatus::Ok => 0,
            ExitStatus::Error => 1,
            ExitStatus::Warning => 2,
        }
    }

    /// Updates this status with a new per-file result.
    pub fn observe_cli_error(&mut self, cause: &DiagnosticCause) {
        match cause {
            DiagnosticCause::Warning(_) => {
                if *self == ExitStatus::Ok {
                    *self = ExitStatus::Warning;
                }
            }
            DiagnosticCause::Error(_) => {
                *self = ExitStatus::Error;
            }
        }
    }
}

/// Result of running the CLI over potentially multiple input files.
#[derive(Debug, Default)]
pub struct Report {
    /// Aggregated exit status.
    pub status: ExitStatus,
    /// All per-file and invocation-level diagnostics encountered.
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// Records a diagnostic and updates aggregated status.
    pub fn record(&mut self, cause: DiagnosticCause, program: &str, file: Option<&str>) {
        self.status.observe_cli_error(&cause);
        self.diagnostics.push(Diagnostic::new(cause, program, file));
    }
}

/// A structured CLI diagnostic that preserves the underlying failure and context.
///
/// This is used to implement `-q/-qq` output suppression while keeping rich
/// error context (program name and input file).
#[derive(Debug)]
pub struct Diagnostic {
    /// Program name to prefix in error output (e.g. "xz", "unxz").
    pub program: String,
    /// Input file path, or `None` for stdin.
    pub file: Option<String>,
    /// Underlying diagnostic cause produced by processing.
    pub cause: DiagnosticCause,
}

impl Diagnostic {
    /// Creates a new [`Diagnostic`] by wrapping a diagnostic cause with program and file context.
    ///
    /// # Parameters
    ///
    /// * `cause` - The underlying diagnostic cause to wrap
    /// * `program` - Program name to include in error messages (e.g., "xz", "unxz", "xzcat")
    /// * `file` - Optional file path associated with the error. Use `None` for stdin or
    ///   when no specific file is associated with the error.
    ///
    /// # Returns
    ///
    /// Returns a new [`Diagnostic`] instance with the provided context.
    pub fn new(cause: DiagnosticCause, program: &str, file: Option<&str>) -> Self {
        Self {
            program: program.to_string(),
            file: file.map(String::from),
            cause,
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.file.as_deref() {
            Some(file) => write!(f, "{}: {}: {}", self.program, file, self.cause),
            None => write!(f, "{}: (stdin): {}", self.program, self.cause),
        }
    }
}

impl std::error::Error for Diagnostic {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Formats a diagnostic message for stderr, respecting `-q/-qq`.
///
/// # Parameters
///
/// - `quiet`: Quiet level (as counted by `-q` occurrences).
/// - `diagnostic`: Diagnostic returned by the CLI runner.
///
/// # Returns
///
/// Returns `None` when the message should be suppressed by `quiet`,
/// otherwise returns a formatted single-line message suitable for stderr.
pub fn format_diagnostic_for_stderr(quiet: u8, diagnostic: &Diagnostic) -> Option<String> {
    if quiet >= 2 || quiet >= 1 && diagnostic.cause.severity() == Severity::Warning {
        return None;
    }

    Some(diagnostic.to_string())
}

/// Warning conditions for XZ CLI operations.
///
/// These are non-fatal conditions that upstream `xz` typically reports as a
/// warning/notice.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Warning {
    /// Input file lacks recognized compression extension
    #[error("Filename has an unknown suffix, skipping")]
    InvalidExtension {
        /// Path to the input file
        path: PathBuf,
    },

    /// Input file already has the target suffix
    #[error("Already has `{}` suffix, skipping", suffix)]
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
    #[error("{source}")]
    OpenInput {
        /// Underlying I/O error
        #[source]
        source: IoErrorNoCode,
    },

    /// Failed to create output file
    #[error("{}: {source}", path.display())]
    CreateOutput {
        /// Path to the output file
        path: PathBuf,
        /// Underlying I/O error
        #[source]
        source: IoErrorNoCode,
    },

    /// Output file already exists
    #[error("{}: Output file already exists", path.display())]
    OutputExists {
        /// Path to the existing file
        path: PathBuf,
    },

    /// Cannot determine output filename
    #[error("Cannot determine output filename")]
    InvalidOutputFilename {
        /// Path to the input file
        path: PathBuf,
    },

    /// Compression operation failed
    #[error("{message}")]
    Compression {
        /// Error message from liblzma
        message: String,
    },

    /// Decompression operation failed
    #[error("{message}")]
    Decompression {
        /// Error message from liblzma
        message: String,
    },

    /// Invalid compression level
    #[error("Unsupported preset: {level}")]
    InvalidCompressionLevel {
        /// The invalid level value
        level: u32,
    },

    /// Invalid option combination or value.
    #[error("{message}")]
    InvalidOption {
        /// Error message describing why the option is invalid.
        message: String,
    },

    /// Thread count too large
    #[error("The number of threads must not exceed {}", u32::MAX)]
    InvalidThreadCount {
        /// The invalid thread count
        count: usize,
    },

    /// Failed to remove input file
    #[error("Cannot remove: {source}")]
    RemoveFile {
        /// Underlying I/O error
        #[source]
        source: IoErrorNoCode,
    },

    /// Invalid memory limit format
    #[error("Invalid memory limit: {0}")]
    InvalidMemoryLimit(String),

    /// Failed to extract file information
    #[error("File format not recognized ({message})")]
    FileInfoExtraction {
        /// Path to the file
        path: String,
        /// Error message
        message: String,
    },

    /// List mode does not support reading from stdin.
    #[error("--list does not support reading from standard input")]
    ListModeStdinUnsupported,

    /// Failed to write to stdout/stderr.
    #[error("{source}")]
    WriteOutput {
        /// Underlying I/O error.
        #[source]
        source: IoErrorNoCode,
    },
}

/// Specialized `Result` type for XZ CLI operations.
pub type Result<T> = std::result::Result<T, DiagnosticCause>;

/// This represents both "real" failures and warning/notice conditions.
#[derive(Debug, Error)]
pub enum DiagnosticCause {
    /// Warning/notice condition.
    #[error(transparent)]
    Warning(#[from] Warning),

    /// Real failure condition.
    #[error(transparent)]
    Error(#[from] Error),
}

impl DiagnosticCause {
    /// Returns the severity for this diagnostic.
    pub const fn severity(&self) -> Severity {
        match self {
            DiagnosticCause::Warning(_) => Severity::Warning,
            DiagnosticCause::Error(_) => Severity::Error,
        }
    }

    /// Returns a reference to the warning if this error represents a warning/notice.
    pub fn as_warning(&self) -> Option<&Warning> {
        match self {
            DiagnosticCause::Warning(w) => Some(w),
            DiagnosticCause::Error(_) => None,
        }
    }

    /// Returns a reference to the underlying "real" error, if any.
    pub fn as_error(&self) -> Option<&Error> {
        match self {
            DiagnosticCause::Warning(_) => None,
            DiagnosticCause::Error(e) => Some(e),
        }
    }
}
