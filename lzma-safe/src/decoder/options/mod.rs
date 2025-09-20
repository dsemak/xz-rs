//! Decoder configuration shared by the safe wrappers.

mod flags;

pub use flags::Flags;

/// Configuration passed to `lzma_stream_decoder_mt` and friends.
pub struct Options {
    /// Number of worker threads (1 means single-threaded).
    pub threads: u32,

    /// Soft memory limit; the decoder may fall back to fewer threads when exceeded.
    pub memlimit: u64,

    /// Hard memory limit that aborts the operation when exceeded.
    pub memlimit_stop: u64,

    /// Behavioural flags, see [`Flags`].
    pub flags: Flags,

    /// Timeout in milliseconds; `0` disables timeouts.
    pub timeout: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            threads: 1,
            memlimit: u64::MAX,
            memlimit_stop: u64::MAX,
            flags: Flags::empty(),
            timeout: 0,
        }
    }
}

impl std::fmt::Debug for Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecoderOptions")
            .field("threads", &self.threads)
            .field("memlimit", &self.memlimit)
            .field("memlimit_stop", &self.memlimit_stop)
            .field("flags", &self.flags)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl Options {
    /// Set the soft memory limit in bytes.
    #[must_use]
    pub fn with_memlimit(mut self, memlimit: u64) -> Self {
        self.memlimit = memlimit;
        self
    }

    /// Set decoder flags.
    #[must_use]
    pub fn with_flags(mut self, flags: Flags) -> Self {
        self.flags = flags;
        self
    }

    /// Convert to the raw `lzma_mt` structure expected by liblzma.
    pub(crate) fn to_lzma_options(&self) -> liblzma_sys::lzma_mt {
        // SAFETY: lzma_mt is a POD struct; zeroed then filled with required fields.
        let mut options = unsafe { std::mem::zeroed::<liblzma_sys::lzma_mt>() };

        // Set only the required fields; leave others at their default zeroed values.
        options.threads = self.threads;
        options.timeout = self.timeout;
        options.flags = self.flags.to_liblzma_flags();
        options.memlimit_threading = self.memlimit;
        options.memlimit_stop = self.memlimit_stop;

        options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test conversion to `lzma_mt` sets all fields correctly.
    #[test]
    fn to_lzma_options_sets_expected_fields() {
        let flags = Flags::NO_CHECK | Flags::CONCATENATED;
        let options = Options {
            threads: 3,
            memlimit: 32 * 1024 * 1024,
            memlimit_stop: 64 * 1024 * 1024,
            flags,
            timeout: 100,
        };

        let mt = options.to_lzma_options();
        assert_eq!(mt.threads, 3);
        assert_eq!(mt.timeout, 100);
        assert_eq!(mt.flags, flags.to_liblzma_flags());
        assert_eq!(mt.memlimit_threading, 32 * 1024 * 1024);
        assert_eq!(mt.memlimit_stop, 64 * 1024 * 1024);
        assert!(mt.filters.is_null());
        assert!(mt.reserved_enum1 == liblzma_sys::lzma_reserved_enum_LZMA_RESERVED_ENUM);
        assert!(mt.reserved_enum2 == liblzma_sys::lzma_reserved_enum_LZMA_RESERVED_ENUM);
        assert!(mt.reserved_enum3 == liblzma_sys::lzma_reserved_enum_LZMA_RESERVED_ENUM);
        assert!(mt.reserved_int1 == 0);
        assert!(mt.reserved_int2 == 0);
        assert!(mt.reserved_int3 == 0);
        assert!(mt.reserved_int4 == 0);
        assert!(mt.reserved_int7 == 0);
        assert!(mt.reserved_int8 == 0);
        assert!(mt.reserved_ptr1.is_null());
        assert!(mt.reserved_ptr2.is_null());
        assert!(mt.reserved_ptr3.is_null());
        assert!(mt.reserved_ptr4.is_null());
    }
}
