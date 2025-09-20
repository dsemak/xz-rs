//! Error types used by the safe liblzma wrapper.

use std::fmt;

/// Type alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// Error values returned by encoder/decoder operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Stream reached `LZMA_STREAM_END`; treated as success by callers.
    StreamEnd,

    /// Memory allocation failed (`LZMA_MEM_ERROR`).
    MemError,

    /// Configured memory limit was exceeded (`LZMA_MEMLIMIT_ERROR`).
    MemLimitError,

    /// Input bytes are not recognised (`LZMA_FORMAT_ERROR`).
    FormatError,

    /// Invalid encoder/decoder options (`LZMA_OPTIONS_ERROR`).
    OptionsError,

    /// Corrupted input (`LZMA_DATA_ERROR`).
    DataError,

    /// Not enough output space to make progress (`LZMA_BUF_ERROR`).
    BufError,

    /// Misuse of the liblzma API (`LZMA_PROG_ERROR`).
    ProgError,

    /// Integrity check type is not supported (`LZMA_UNSUPPORTED_CHECK`).
    UnsupportedCheck,

    /// Fallback for error codes not known to this wrapper.
    Unknown(liblzma_sys::lzma_ret),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::StreamEnd => write!(f, "End of stream reached"),
            Error::MemError => write!(f, "Memory allocation failed"),
            Error::MemLimitError => write!(f, "Memory usage limit was reached"),
            Error::FormatError => write!(f, "File format not recognized"),
            Error::OptionsError => write!(f, "Invalid or unsupported options"),
            Error::DataError => write!(f, "Data is corrupt"),
            Error::BufError => write!(f, "No progress is possible"),
            Error::ProgError => write!(f, "Programming error"),
            Error::UnsupportedCheck => write!(f, "Integrity check type is not supported"),
            Error::Unknown(code) => write!(f, "Unknown error code: {code}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<liblzma_sys::lzma_ret> for Error {
    fn from(ret: liblzma_sys::lzma_ret) -> Error {
        match ret {
            liblzma_sys::lzma_ret_LZMA_OK => unreachable!("LZMA_OK is treated as success"),
            liblzma_sys::lzma_ret_LZMA_STREAM_END => Error::StreamEnd,
            liblzma_sys::lzma_ret_LZMA_MEM_ERROR => Error::MemError,
            liblzma_sys::lzma_ret_LZMA_MEMLIMIT_ERROR => Error::MemLimitError,
            liblzma_sys::lzma_ret_LZMA_FORMAT_ERROR => Error::FormatError,
            liblzma_sys::lzma_ret_LZMA_OPTIONS_ERROR => Error::OptionsError,
            liblzma_sys::lzma_ret_LZMA_DATA_ERROR => Error::DataError,
            liblzma_sys::lzma_ret_LZMA_BUF_ERROR => Error::BufError,
            liblzma_sys::lzma_ret_LZMA_PROG_ERROR => Error::ProgError,
            liblzma_sys::lzma_ret_LZMA_UNSUPPORTED_CHECK => Error::UnsupportedCheck,
            other => Error::Unknown(other),
        }
    }
}

impl Error {
    /// Return the raw `lzma_ret` code for the current variant.
    pub fn to_raw(self) -> liblzma_sys::lzma_ret {
        match self {
            Error::StreamEnd => liblzma_sys::lzma_ret_LZMA_STREAM_END,
            Error::MemError => liblzma_sys::lzma_ret_LZMA_MEM_ERROR,
            Error::MemLimitError => liblzma_sys::lzma_ret_LZMA_MEMLIMIT_ERROR,
            Error::FormatError => liblzma_sys::lzma_ret_LZMA_FORMAT_ERROR,
            Error::OptionsError => liblzma_sys::lzma_ret_LZMA_OPTIONS_ERROR,
            Error::DataError => liblzma_sys::lzma_ret_LZMA_DATA_ERROR,
            Error::BufError => liblzma_sys::lzma_ret_LZMA_BUF_ERROR,
            Error::ProgError => liblzma_sys::lzma_ret_LZMA_PROG_ERROR,
            Error::UnsupportedCheck => liblzma_sys::lzma_ret_LZMA_UNSUPPORTED_CHECK,
            Error::Unknown(code) => code,
        }
    }
}

/// Translate a `liblzma` status code into a `Result`.
pub(crate) fn result_from_lzma_ret<T>(ret: liblzma_sys::lzma_ret, value: T) -> Result<T> {
    if ret == liblzma_sys::lzma_ret_LZMA_OK {
        Ok(value)
    } else {
        Err(ret.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Test conversion from all known `lzma_ret` codes to Error variants.
    #[test]
    fn test_lzma_error_from_all_known_codes() {
        let cases = [
            (liblzma_sys::lzma_ret_LZMA_STREAM_END, Error::StreamEnd),
            (liblzma_sys::lzma_ret_LZMA_MEM_ERROR, Error::MemError),
            (
                liblzma_sys::lzma_ret_LZMA_MEMLIMIT_ERROR,
                Error::MemLimitError,
            ),
            (liblzma_sys::lzma_ret_LZMA_FORMAT_ERROR, Error::FormatError),
            (
                liblzma_sys::lzma_ret_LZMA_OPTIONS_ERROR,
                Error::OptionsError,
            ),
            (liblzma_sys::lzma_ret_LZMA_DATA_ERROR, Error::DataError),
            (liblzma_sys::lzma_ret_LZMA_BUF_ERROR, Error::BufError),
            (liblzma_sys::lzma_ret_LZMA_PROG_ERROR, Error::ProgError),
            (
                liblzma_sys::lzma_ret_LZMA_UNSUPPORTED_CHECK,
                Error::UnsupportedCheck,
            ),
        ];

        for &(code, ref expected_variant) in &cases {
            let err = Error::from(code);
            assert_eq!(&err, expected_variant, "Failed for code: {code}");
        }
    }

    /// Test conversion from unknown `lzma_ret` code to [`Error::Unknown`].
    #[test]
    fn test_lzma_error_from_unknown_code() {
        let unknown_code = 12345;
        let err = Error::from(unknown_code);
        match err {
            Error::Unknown(code) => assert_eq!(code, unknown_code),
            _ => panic!("Expected Unknown variant"),
        }
    }

    /// Test that [`Error::from`] panics on `LZMA_OK`.
    #[test]
    #[should_panic(expected = "LZMA_OK is treated as success")]
    fn test_lzma_error_from_ok_panics() {
        let _ = Error::from(liblzma_sys::lzma_ret_LZMA_OK);
    }

    /// Test that [`Error::to_raw`] returns the correct `lzma_ret` code for each Error variant.
    #[test]
    fn test_lzma_error_to_raw_all_variants() {
        let cases = [
            (Error::StreamEnd, liblzma_sys::lzma_ret_LZMA_STREAM_END),
            (Error::MemError, liblzma_sys::lzma_ret_LZMA_MEM_ERROR),
            (
                Error::MemLimitError,
                liblzma_sys::lzma_ret_LZMA_MEMLIMIT_ERROR,
            ),
            (Error::FormatError, liblzma_sys::lzma_ret_LZMA_FORMAT_ERROR),
            (
                Error::OptionsError,
                liblzma_sys::lzma_ret_LZMA_OPTIONS_ERROR,
            ),
            (Error::DataError, liblzma_sys::lzma_ret_LZMA_DATA_ERROR),
            (Error::BufError, liblzma_sys::lzma_ret_LZMA_BUF_ERROR),
            (Error::ProgError, liblzma_sys::lzma_ret_LZMA_PROG_ERROR),
            (
                Error::UnsupportedCheck,
                liblzma_sys::lzma_ret_LZMA_UNSUPPORTED_CHECK,
            ),
            (Error::Unknown(42), 42),
        ];

        for &(ref variant, code) in &cases {
            assert_eq!(variant.to_raw(), code, "Failed for variant: {variant:?}");
        }
    }

    /// Test [`result_from_lzma_ret`] returns Ok for `LZMA_OK` and Err for error codes.
    #[test]
    fn test_result_from_lzma_ret_behavior() {
        let value = 123;
        let ok = result_from_lzma_ret(liblzma_sys::lzma_ret_LZMA_OK, value);
        assert_eq!(ok, Ok(value));

        let err_code = liblzma_sys::lzma_ret_LZMA_DATA_ERROR;
        let err = result_from_lzma_ret::<i32>(err_code, value);
        assert!(matches!(err, Err(Error::DataError)));
    }

    /// Test roundtrip: code -> Error -> code for all known and unknown codes.
    #[test]
    fn test_lzma_error_roundtrip_all_codes() {
        let codes = [
            liblzma_sys::lzma_ret_LZMA_STREAM_END,
            liblzma_sys::lzma_ret_LZMA_MEM_ERROR,
            liblzma_sys::lzma_ret_LZMA_MEMLIMIT_ERROR,
            liblzma_sys::lzma_ret_LZMA_FORMAT_ERROR,
            liblzma_sys::lzma_ret_LZMA_OPTIONS_ERROR,
            liblzma_sys::lzma_ret_LZMA_DATA_ERROR,
            liblzma_sys::lzma_ret_LZMA_BUF_ERROR,
            liblzma_sys::lzma_ret_LZMA_PROG_ERROR,
            liblzma_sys::lzma_ret_LZMA_UNSUPPORTED_CHECK,
            99999, // unknown code
        ];

        for &code in &codes {
            let error = Error::from(code);
            assert_eq!(error.to_raw(), code, "Roundtrip failed for code: {code}");
        }
    }
}
