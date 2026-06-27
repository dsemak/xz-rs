//! Compression configuration builder.

use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;

use lzma_safe::encoder::options::Options as EncoderMtOptions;
use lzma_safe::{AloneEncoder, Encoder, RawEncoder, Stream};

pub use lzma_safe::encoder::options::{
    BcjOptions, Compression, DeltaOptions, FilterConfig, FilterOptions, FilterType, IntegrityCheck,
    LzmaOptions,
};

/// LZMA1 encoder tuning options exposed for `.lzma` (`LZMA_Alone`) usage.
pub mod lzma1 {
    pub use lzma_safe::encoder::options::{Lzma1Options, MatchFinder, Mode};
}

use crate::compression::BuiltEncoder;
use crate::config::{DecodeMode, EncodeFormat};
use crate::error::{Error, Result};
use crate::threading::{sanitize_threads, Threading};
use crate::util::{duration_to_timeout, DEFAULT_INPUT_BUFFER, DEFAULT_OUTPUT_BUFFER};

/// Configuration builder for XZ compression operations.
#[derive(Debug, Clone)]
pub struct Options {
    level: Compression,
    check: IntegrityCheck,
    threads: Threading,
    block_size: Option<NonZeroU64>,
    timeout: Option<Duration>,
    filters: Vec<FilterConfig>,
    format: EncodeFormat,
    lzma1: Option<lzma1::Lzma1Options>,
    input_buffer_size: NonZeroUsize,
    output_buffer_size: NonZeroUsize,
    xz_mt_encoder: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            level: Compression::Level6,
            check: IntegrityCheck::Crc64,
            threads: Threading::Auto,
            block_size: None,
            timeout: None,
            filters: Vec::new(),
            format: EncodeFormat::Xz,
            lzma1: None,
            input_buffer_size: NonZeroUsize::new(DEFAULT_INPUT_BUFFER).unwrap(),
            output_buffer_size: NonZeroUsize::new(DEFAULT_OUTPUT_BUFFER).unwrap(),
            xz_mt_encoder: false,
        }
    }
}

impl Options {
    #[must_use]
    pub fn with_level(mut self, level: Compression) -> Self {
        self.level = level;
        self
    }

    #[must_use]
    pub fn with_check(mut self, check: IntegrityCheck) -> Self {
        self.check = check;
        self
    }

    #[must_use]
    pub fn with_threads(mut self, threads: Threading) -> Self {
        self.threads = threads;
        self
    }

    #[must_use]
    pub fn with_block_size(mut self, block_size: Option<NonZeroU64>) -> Self {
        self.block_size = block_size;
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub fn with_filters(mut self, filters: Vec<FilterConfig>) -> Self {
        self.filters = filters;
        self
    }

    #[must_use]
    pub fn with_format(mut self, format: EncodeFormat) -> Self {
        self.format = format;
        self
    }

    #[must_use]
    pub fn with_lzma1_options(mut self, options: Option<lzma1::Lzma1Options>) -> Self {
        self.lzma1 = options;
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

    #[must_use]
    pub fn with_xz_mt_encoder(mut self, mt_encoder: bool) -> Self {
        self.xz_mt_encoder = mt_encoder;
        self
    }

    pub(crate) fn build_encoder(&self) -> Result<BuiltEncoder> {
        match self.format {
            EncodeFormat::Xz => self.build_xz_encoder().map(BuiltEncoder::Xz),
            EncodeFormat::Lzma => self.build_lzma_encoder().map(BuiltEncoder::Lzma),
            EncodeFormat::Raw => self.build_raw_encoder().map(BuiltEncoder::Raw),
        }
    }

    fn build_xz_encoder(&self) -> Result<Encoder> {
        let threads = match sanitize_threads(self.threads) {
            Ok(count) => count.max(1),
            Err(Error::InvalidThreadCount { maximum, .. }) => maximum.max(1),
            Err(other) => return Err(other),
        };
        let stream = Stream::default();

        if self.xz_mt_encoder {
            let mut options = EncoderMtOptions::default()
                .with_level(self.level)
                .with_check(self.check)
                .with_threads(threads);

            if let Some(block) = self.block_size {
                options = options.with_block_size(block.get());
            }

            if let Some(timeout) = self.timeout {
                options = options.with_timeout(duration_to_timeout(timeout));
            }

            if !self.filters.is_empty() {
                options = options.with_filters(self.filters.clone());
            }

            return Encoder::new_mt(options, stream).map_err(Error::from);
        }

        if !self.filters.is_empty() {
            return Encoder::new_stream(self.filters.clone(), self.check, stream)
                .map_err(Error::from);
        }

        if threads <= 1 && self.block_size.is_none() && self.timeout.is_none() {
            return Encoder::new(self.level, self.check, stream).map_err(Error::from);
        }

        let mut options = EncoderMtOptions::default()
            .with_level(self.level)
            .with_check(self.check)
            .with_threads(threads);

        if let Some(block) = self.block_size {
            options = options.with_block_size(block.get());
        }

        if let Some(timeout) = self.timeout {
            options = options.with_timeout(duration_to_timeout(timeout));
        }

        if !self.filters.is_empty() {
            options = options.with_filters(self.filters.clone());
        }

        Encoder::new_mt(options, stream).map_err(Error::from)
    }

    fn build_lzma_encoder(&self) -> Result<AloneEncoder> {
        if self.check != IntegrityCheck::None {
            return Err(Error::InvalidOption(
                "integrity checks are not supported in .lzma format".into(),
            ));
        }
        if let Threading::Exact(requested) = self.threads {
            if requested > 1 {
                return Err(Error::ThreadingUnsupported {
                    requested,
                    mode: DecodeMode::Lzma,
                });
            }
        }
        if self.block_size.is_some() {
            return Err(Error::InvalidOption(
                "block size is not supported in .lzma format".into(),
            ));
        }
        if self.timeout.is_some() {
            return Err(Error::InvalidOption(
                "timeout is not supported in .lzma format".into(),
            ));
        }
        if !self.filters.is_empty() {
            return Err(Error::InvalidOption(
                "custom filter chains are not supported in .lzma format".into(),
            ));
        }

        let options = match self.lzma1.clone() {
            Some(v) => v,
            None => lzma1::Lzma1Options::from_preset(self.level).map_err(Error::from)?,
        };
        AloneEncoder::new(options, Stream::default()).map_err(Error::from)
    }

    fn build_raw_encoder(&self) -> Result<RawEncoder> {
        if self.check != IntegrityCheck::None {
            return Err(Error::InvalidOption(
                "integrity checks are not supported in raw format".into(),
            ));
        }
        if let Threading::Exact(requested) = self.threads {
            if requested > 1 {
                return Err(Error::InvalidOption(
                    "multi-threaded compression is not supported in raw format".into(),
                ));
            }
        }
        if self.block_size.is_some() {
            return Err(Error::InvalidOption(
                "block size is not supported in raw format".into(),
            ));
        }
        if self.timeout.is_some() {
            return Err(Error::InvalidOption(
                "timeout is not supported in raw format".into(),
            ));
        }
        if !self.filters.is_empty() {
            return Err(Error::InvalidOption(
                "custom filter chains are not supported in raw format".into(),
            ));
        }

        let options = self.lzma1.clone().ok_or_else(|| {
            Error::InvalidOption("raw format requires explicit LZMA1 filter options".into())
        })?;
        RawEncoder::new_lzma1(options, Stream::default()).map_err(Error::from)
    }

    pub(crate) fn input_capacity(&self) -> usize {
        self.input_buffer_size.get()
    }

    pub(crate) fn output_capacity(&self) -> usize {
        self.output_buffer_size.get()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn defaults() {
        let options = Options::default();
        assert_eq!(options.input_capacity(), DEFAULT_INPUT_BUFFER);
        assert_eq!(options.output_capacity(), DEFAULT_OUTPUT_BUFFER);
    }

    #[test]
    fn buffer_sizes_follow_configuration() {
        let input_size = NonZeroUsize::new(8 * 1024).unwrap();
        let output_size = NonZeroUsize::new(16 * 1024).unwrap();
        let options = Options::default()
            .with_input_buffer_size(input_size)
            .with_output_buffer_size(output_size);

        assert_eq!(options.input_capacity(), input_size.get());
        assert_eq!(options.output_capacity(), output_size.get());
    }

    #[test]
    fn zero_thread_request_is_clamped() {
        let options = Options::default().with_threads(Threading::Exact(0));
        options
            .build_encoder()
            .expect("sanitized thread count should construct encoder");
    }

    #[test]
    fn single_threaded_builds_successfully() {
        Options::default()
            .with_threads(Threading::Exact(1))
            .build_encoder()
            .expect("single-threaded encoder should build successfully");
    }

    #[test]
    fn auto_threading_builds_successfully() {
        Options::default()
            .with_threads(Threading::Auto)
            .build_encoder()
            .expect("auto-threaded encoder should build successfully");
    }

    #[test]
    fn builder_pattern() {
        let _options = Options::default()
            .with_level(Compression::Level9)
            .with_check(IntegrityCheck::Sha256)
            .with_threads(Threading::Exact(4))
            .with_block_size(Some(NonZeroU64::new(1024 * 1024).unwrap()))
            .with_timeout(Some(Duration::from_secs(30)))
            .with_filters(vec![]);
    }

    #[test]
    fn clone_works() {
        let original = Options::default().with_level(Compression::Level9);
        let cloned = original.clone();

        assert_eq!(original.input_capacity(), cloned.input_capacity());
    }
}
