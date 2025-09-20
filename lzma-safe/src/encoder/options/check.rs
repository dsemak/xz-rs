//! Integrity check algorithms supported by liblzma.

use crate::Error;

/// Enum mirroring `lzma_check` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityCheck {
    /// Do not embed a check value.
    None,

    /// CRC32 checksum.
    Crc32,

    /// CRC64 checksum.
    Crc64,

    /// SHA-256 hash.
    Sha256,
}

impl IntegrityCheck {
    /// The size in bytes of the integrity check value for each variant.
    const LZMA_CHECK_NONE_SIZE: usize = 0;
    const LZMA_CHECK_CRC32_SIZE: usize = 4;
    const LZMA_CHECK_CRC64_SIZE: usize = 8;
    const LZMA_CHECK_SHA256_SIZE: usize = 32;

    /// Return the number of bytes required to store the check value.
    pub fn size(&self) -> usize {
        match self {
            IntegrityCheck::None => Self::LZMA_CHECK_NONE_SIZE,
            IntegrityCheck::Crc32 => Self::LZMA_CHECK_CRC32_SIZE,
            IntegrityCheck::Crc64 => Self::LZMA_CHECK_CRC64_SIZE,
            IntegrityCheck::Sha256 => Self::LZMA_CHECK_SHA256_SIZE,
        }
    }
}

impl From<IntegrityCheck> for liblzma_sys::lzma_check {
    /// Converts an [`IntegrityCheck`] variant to the corresponding `lzma_check` constant.
    ///
    /// This conversion is used internally to pass integrity check settings to
    /// the liblzma C API. The conversion is always safe and cannot fail.
    fn from(check: IntegrityCheck) -> Self {
        match check {
            IntegrityCheck::None => liblzma_sys::lzma_check_LZMA_CHECK_NONE,
            IntegrityCheck::Crc32 => liblzma_sys::lzma_check_LZMA_CHECK_CRC32,
            IntegrityCheck::Crc64 => liblzma_sys::lzma_check_LZMA_CHECK_CRC64,
            IntegrityCheck::Sha256 => liblzma_sys::lzma_check_LZMA_CHECK_SHA256,
        }
    }
}

impl TryFrom<liblzma_sys::lzma_check> for IntegrityCheck {
    type Error = Error;

    /// Attempts to convert a raw `lzma_check` value into an [`IntegrityCheck`] variant.
    ///
    /// This conversion is used when reading integrity check information from
    /// compressed streams or liblzma API responses.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedCheck`] if the value does not correspond to
    /// a supported check type. This can happen with newer liblzma versions that
    /// support check types not yet known to this crate.
    fn try_from(check: liblzma_sys::lzma_check) -> std::result::Result<Self, Self::Error> {
        match check {
            liblzma_sys::lzma_check_LZMA_CHECK_NONE => Ok(IntegrityCheck::None),
            liblzma_sys::lzma_check_LZMA_CHECK_CRC32 => Ok(IntegrityCheck::Crc32),
            liblzma_sys::lzma_check_LZMA_CHECK_CRC64 => Ok(IntegrityCheck::Crc64),
            liblzma_sys::lzma_check_LZMA_CHECK_SHA256 => Ok(IntegrityCheck::Sha256),
            _ => Err(Error::UnsupportedCheck),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test check sizes match expected byte counts.
    #[test]
    fn size_matches_expected_values() {
        assert_eq!(IntegrityCheck::None.size(), 0);
        assert_eq!(IntegrityCheck::Crc32.size(), 4);
        assert_eq!(IntegrityCheck::Crc64.size(), 8);
        assert_eq!(IntegrityCheck::Sha256.size(), 32);
    }

    /// Test [`IntegrityCheck::try_from`] conversion round-trips for valid values.
    #[test]
    fn try_from_round_trips_valid_values() {
        assert_eq!(
            IntegrityCheck::try_from(liblzma_sys::lzma_check_LZMA_CHECK_NONE).unwrap(),
            IntegrityCheck::None
        );
        assert!(IntegrityCheck::try_from(42).is_err());
    }
}
