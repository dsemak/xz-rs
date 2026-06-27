//! Decompression configuration builder.

use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;

use lzma_safe::decoder::options::{Flags as DecoderFlags, Options as DecoderMtOptions};
use lzma_safe::{Decoder, RawDecoder, Stream};

pub use crate::compression::lzma1;

use crate::compression::lzma1::Lzma1Options;
use crate::config::{DecodeMode, UnknownInputPolicy};
use crate::decompression::BuiltDecoder;
use crate::error::{Error, Result};
use crate::threading::{sanitize_threads, Threading};
use crate::util::{duration_to_timeout, DEFAULT_INPUT_BUFFER, DEFAULT_OUTPUT_BUFFER};

/// Configuration builder for XZ decompression operations with security-focused defaults.
#[derive(Debug, Clone)]
pub struct Options {
    threads: Threading,
    memlimit: NonZeroU64,
    memlimit_stop: Option<NonZeroU64>,
    flags: DecoderFlags,
    mode: DecodeMode,
    unknown_input_policy: UnknownInputPolicy,
    raw_lzma1: Option<Lzma1Options>,
    timeout: Option<Duration>,
    input_buffer_size: NonZeroUsize,
    output_buffer_size: NonZeroUsize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            threads: Threading::Auto,
            memlimit: NonZeroU64::new(256 * 1024 * 1024).unwrap(),
            memlimit_stop: None,
            flags: DecoderFlags::empty(),
            mode: DecodeMode::Auto,
            unknown_input_policy: UnknownInputPolicy::Error,
            raw_lzma1: None,
            timeout: None,
            input_buffer_size: NonZeroUsize::new(DEFAULT_INPUT_BUFFER).unwrap(),
            output_buffer_size: NonZeroUsize::new(DEFAULT_OUTPUT_BUFFER).unwrap(),
        }
    }
}

impl Options {
    #[must_use]
    pub fn with_threads(mut self, threads: Threading) -> Self {
        self.threads = threads;
        self
    }

    #[must_use]
    pub fn with_memlimit(mut self, limit: NonZeroU64) -> Self {
        self.memlimit = limit;
        self
    }

    #[must_use]
    pub fn with_memlimit_stop(mut self, limit: Option<NonZeroU64>) -> Self {
        self.memlimit_stop = limit;
        self
    }

    #[must_use]
    pub fn with_flags(mut self, flags: DecoderFlags) -> Self {
        self.flags = flags;
        self
    }

    #[must_use]
    pub fn with_mode(mut self, mode: DecodeMode) -> Self {
        self.mode = mode;
        self
    }

    #[must_use]
    pub fn with_unknown_input_policy(mut self, policy: UnknownInputPolicy) -> Self {
        self.unknown_input_policy = policy;
        self
    }

    #[must_use]
    pub fn with_raw_lzma1_options(mut self, options: Option<Lzma1Options>) -> Self {
        self.raw_lzma1 = options;
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub fn with_input_buffer_size(mut self, size: NonZeroUsize) -> Self {
        self.input_buffer_size = size;
        self
    }

    #[must_use]
    pub fn with_output_buffer_size(mut self, size: NonZeroUsize) -> Self {
        self.output_buffer_size = size;
        self
    }

    pub(crate) fn build_decoder(&self) -> Result<BuiltDecoder> {
        let memlimit = self.memlimit.get();
        let memlimit_stop = self
            .memlimit_stop
            .map_or(self.memlimit.get(), NonZeroU64::get);

        if memlimit_stop < memlimit {
            return Err(Error::InvalidOption(
                "memlimit_stop must be greater than or equal to memlimit".into(),
            ));
        }

        let stream = Stream::default();

        match self.mode {
            DecodeMode::Auto => {
                if let Threading::Exact(requested) = self.threads {
                    if requested > 1 {
                        return Err(Error::ThreadingUnsupported {
                            requested,
                            mode: DecodeMode::Auto,
                        });
                    }
                }

                Decoder::new_auto(memlimit, self.flags, stream)
                    .map(BuiltDecoder::Standard)
                    .map_err(Error::from)
            }
            DecodeMode::Xz => {
                let threads = match sanitize_threads(self.threads) {
                    Ok(count) => count.max(1),
                    Err(Error::InvalidThreadCount { maximum, .. }) => maximum.max(1),
                    Err(other) => return Err(other),
                };

                let options = DecoderMtOptions {
                    threads,
                    memlimit,
                    memlimit_stop,
                    flags: self.flags,
                    timeout: self.timeout.map_or(0, duration_to_timeout),
                };

                Decoder::new_mt(options, stream)
                    .map(BuiltDecoder::Standard)
                    .map_err(Error::from)
            }
            DecodeMode::Lzma => {
                if let Threading::Exact(requested) = self.threads {
                    if requested > 1 {
                        return Err(Error::ThreadingUnsupported {
                            requested,
                            mode: DecodeMode::Lzma,
                        });
                    }
                }

                Decoder::new_alone(memlimit, stream)
                    .map(BuiltDecoder::Standard)
                    .map_err(Error::from)
            }
            DecodeMode::Raw => {
                if let Threading::Exact(requested) = self.threads {
                    if requested > 1 {
                        return Err(Error::ThreadingUnsupported {
                            requested,
                            mode: DecodeMode::Raw,
                        });
                    }
                }

                let lzma1 = self.raw_lzma1.clone().ok_or_else(|| {
                    Error::InvalidOption(
                        "raw decode mode requires explicit LZMA1 filter options".into(),
                    )
                })?;
                RawDecoder::new_lzma1(memlimit, self.flags, lzma1, stream)
                    .map(BuiltDecoder::Raw)
                    .map_err(Error::from)
            }
        }
    }

    pub(crate) fn input_capacity(&self) -> usize {
        self.input_buffer_size.get()
    }

    pub(crate) fn output_capacity(&self) -> usize {
        self.output_buffer_size.get()
    }

    pub(crate) fn flags(&self) -> DecoderFlags {
        self.flags
    }

    pub(crate) fn mode(&self) -> DecodeMode {
        self.mode
    }

    pub(crate) fn unknown_input_policy(&self) -> UnknownInputPolicy {
        self.unknown_input_policy
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use lzma_safe::decoder::options::Flags;

    use super::*;

    #[test]
    fn defaults() {
        let options = Options::default();
        assert_eq!(options.input_capacity(), DEFAULT_INPUT_BUFFER);
        assert_eq!(options.output_capacity(), DEFAULT_OUTPUT_BUFFER);
        assert_eq!(options.memlimit.get(), 256 * 1024 * 1024);
        assert_eq!(options.threads, Threading::Auto);
        assert_eq!(options.mode, DecodeMode::Auto);
        assert_eq!(options.unknown_input_policy, UnknownInputPolicy::Error);
        assert!(options.flags.is_empty());
    }

    #[test]
    fn buffer_sizes_follow_configuration() {
        let input_size = NonZeroUsize::new(32 * 1024).unwrap();
        let output_size = NonZeroUsize::new(128 * 1024).unwrap();
        let options = Options::default()
            .with_input_buffer_size(input_size)
            .with_output_buffer_size(output_size);

        assert_eq!(options.input_capacity(), input_size.get());
        assert_eq!(options.output_capacity(), output_size.get());
    }

    #[test]
    fn builder_pattern() {
        let memlimit = NonZeroU64::new(128 * 1024 * 1024).unwrap();
        let memlimit_stop = NonZeroU64::new(256 * 1024 * 1024).unwrap();

        let options = Options::default()
            .with_threads(Threading::Exact(2))
            .with_memlimit(memlimit)
            .with_memlimit_stop(Some(memlimit_stop))
            .with_flags(Flags::CONCATENATED)
            .with_mode(DecodeMode::Xz)
            .with_unknown_input_policy(UnknownInputPolicy::Passthrough)
            .with_timeout(Some(Duration::from_secs(10)));

        assert_eq!(options.threads, Threading::Exact(2));
        assert_eq!(options.memlimit, memlimit);
        assert_eq!(options.memlimit_stop, Some(memlimit_stop));
        assert!(options.flags.is_concatenated());
        assert_eq!(options.mode, DecodeMode::Xz);
        assert_eq!(
            options.unknown_input_policy,
            UnknownInputPolicy::Passthrough
        );
        assert_eq!(options.timeout, Some(Duration::from_secs(10)));
    }

    #[test]
    fn memlimit_stop_must_exceed_soft_limit() {
        let options = Options::default()
            .with_memlimit(NonZeroU64::new(2_048).unwrap())
            .with_memlimit_stop(Some(NonZeroU64::new(1_024).unwrap()));

        assert!(matches!(
            options.build_decoder(),
            Err(Error::InvalidOption(_))
        ));
    }

    #[test]
    fn memlimit_stop_equal_to_soft_limit_allowed() {
        let limit = NonZeroU64::new(64 * 1024 * 1024).unwrap();
        let options = Options::default()
            .with_memlimit(limit)
            .with_memlimit_stop(Some(limit));

        let result = options.build_decoder();
        assert!(result.is_ok() || matches!(result, Err(Error::ThreadingUnsupported { .. })));
    }

    #[test]
    fn lzma_mode_rejects_multi_threading() {
        let options = Options::default()
            .with_mode(DecodeMode::Lzma)
            .with_threads(Threading::Exact(2));

        assert!(matches!(
            options.build_decoder(),
            Err(Error::ThreadingUnsupported {
                requested: 2,
                mode: DecodeMode::Lzma
            })
        ));
    }

    #[test]
    fn auto_mode_rejects_explicit_multi_threading() {
        let options = Options::default()
            .with_mode(DecodeMode::Auto)
            .with_threads(Threading::Exact(2));

        assert!(matches!(
            options.build_decoder(),
            Err(Error::ThreadingUnsupported {
                requested: 2,
                mode: DecodeMode::Auto
            })
        ));
    }

    #[test]
    fn auto_mode_accepts_single_threading() {
        Options::default()
            .with_mode(DecodeMode::Auto)
            .with_threads(Threading::Exact(1))
            .build_decoder()
            .expect("Auto mode should accept single threading");
    }

    #[test]
    fn xz_mode_accepts_multi_threading() {
        Options::default()
            .with_mode(DecodeMode::Xz)
            .with_threads(Threading::Exact(2))
            .build_decoder()
            .expect("XZ mode should accept multi-threading");
    }

    #[test]
    fn clone_works() {
        let original = Options::default().with_memlimit(NonZeroU64::new(128 * 1024 * 1024).unwrap());
        let cloned = original.clone();

        assert_eq!(original.memlimit, cloned.memlimit);
        assert_eq!(original.input_capacity(), cloned.input_capacity());
    }
}
