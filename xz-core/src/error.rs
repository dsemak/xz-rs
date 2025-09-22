//! Error types and result handling for XZ compression and decompression operations.

use std::fmt;

use crate::config::DecodeMode;

pub use lzma_safe::Error as BackendError;

/// Result alias using the crate-level [`Error`] type.
pub type Result<T> = std::result::Result<T, Error>;

/// Comprehensive error type covering all failure modes in XZ operations.
#[derive(Debug)]
pub enum Error {
    /// Failure returned by the safe liblzma wrapper.
    Backend(BackendError),

    /// I/O failure while reading input or writing output.
    Io(std::io::Error),

    /// The requested thread count exceeds the safe limit for the host.
    InvalidThreadCount {
        /// Number of threads requested by the user
        requested: u32,
        /// Maximum safe thread count for the current system
        maximum: u32,
    },

    /// Threading is not supported for the selected decoder mode.
    ThreadingUnsupported {
        /// Number of threads requested by the user
        requested: u32,
        /// Decoder mode that doesn't support threading
        mode: DecodeMode,
    },

    /// Invalid option supplied by the caller.
    InvalidOption(String),

    /// The linked liblzma version is known to be compromised.
    CompromisedBackend {
        /// Version string of the compromised liblzma library
        version: String,
    },

    /// Requested buffer could not be allocated.
    AllocationFailed {
        /// Size in bytes of the buffer that failed to allocate
        capacity: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Backend(err) => write!(f, "liblzma backend error: {err}"),
            Error::Io(err) => write!(f, "I/O error: {err}"),
            Error::InvalidThreadCount { requested, maximum } => write!(
                f,
                "requested {requested} threads exceeds safe limit of {maximum}",
            ),
            Error::ThreadingUnsupported { requested, mode } => write!(
                f,
                "threading with {requested} workers is not supported for decoder mode {mode:?}",
            ),
            Error::InvalidOption(message) => write!(f, "invalid option: {message}"),
            Error::CompromisedBackend { version } => write!(
                f,
                "refusing to use compromised liblzma release {version}; update your system",
            ),
            Error::AllocationFailed { capacity } => {
                write!(f, "unable to allocate temporary buffer of {capacity} bytes")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Backend(err) => Some(err),
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<BackendError> for Error {
    fn from(err: BackendError) -> Self {
        Error::Backend(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
