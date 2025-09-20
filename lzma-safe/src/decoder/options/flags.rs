//! Flags that fine-tune the behaviour of the decoder.

/// Wrapper around liblzma's `lzma_decoder_flag` bit-field.
#[derive(Debug, Clone, Copy)]
pub struct Flags(u32);

bitflags::bitflags! {
    impl Flags: u32 {
        /// Report that the input stream lacks an integrity check (`LZMA_TELL_NO_CHECK`).
        const NO_CHECK = 0x01;

        /// Report unsupported check types (`LZMA_TELL_UNSUPPORTED_CHECK`).
        const UNSUPPORTED_CHECK = 0x02;

        /// Emit a status once the integrity check type becomes known (`LZMA_TELL_ANY_CHECK`).
        const ANY_CHECK = 0x04;

        /// Process concatenated `.xz` streams (`LZMA_CONCATENATED`).
        const CONCATENATED = 0x08;

        /// Skip verification of integrity checks (`LZMA_IGNORE_CHECK`).
        const IGNORE_CHECK = 0x10;
    }
}

impl Default for Flags {
    fn default() -> Self {
        Flags::empty()
    }
}

impl Flags {
    /// Check whether `NO_CHECK` is present.
    pub fn is_no_check(&self) -> bool {
        self.contains(Flags::NO_CHECK)
    }

    /// Check whether `UNSUPPORTED_CHECK` is present.
    pub fn is_unsupported_check(&self) -> bool {
        self.contains(Flags::UNSUPPORTED_CHECK)
    }

    /// Check whether `ANY_CHECK` is present.
    pub fn is_any_check(&self) -> bool {
        self.contains(Flags::ANY_CHECK)
    }

    /// Check whether concatenated stream support is enabled.
    pub fn is_concatenated(&self) -> bool {
        self.contains(Flags::CONCATENATED)
    }

    /// Check whether integrity verification is disabled.
    pub fn is_ignore_check(&self) -> bool {
        self.contains(Flags::IGNORE_CHECK)
    }

    /// Expose the raw bit-field expected by liblzma.
    pub fn to_liblzma_flags(&self) -> u32 {
        self.bits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test flag helper methods reflect correct bit values.
    #[test]
    fn flag_helpers_reflect_bits() {
        let flags = Flags::NO_CHECK | Flags::UNSUPPORTED_CHECK | Flags::CONCATENATED;
        assert!(flags.is_no_check());
        assert!(flags.is_unsupported_check());
        assert!(flags.is_concatenated());
        assert!(!flags.is_any_check());
        assert!(!flags.is_ignore_check());
        assert_eq!(flags.to_liblzma_flags(), 0x01 | 0x02 | 0x08);
    }

    /// Test flag helpers for ignore and any check flags.
    #[test]
    fn flag_helpers_for_ignore_and_any() {
        let flags = Flags::ANY_CHECK | Flags::IGNORE_CHECK;
        assert!(flags.is_any_check());
        assert!(flags.is_ignore_check());
    }
}
