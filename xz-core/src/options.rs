//! High-level configuration builders for XZ compression and decompression operations.

use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;

use lzma_safe::decoder::options::{Flags as DecoderFlags, Options as DecoderMtOptions};
use lzma_safe::encoder::options::Lzma1Options;
use lzma_safe::encoder::options::Options as EncoderMtOptions;
use lzma_safe::{AloneEncoder, Decoder, Encoder, Stream};

pub use lzma_safe::decoder::options::Flags;
pub use lzma_safe::encoder::options::{
    Compression, FilterConfig, FilterOptions, FilterType, IntegrityCheck,
};

use crate::config::DecodeMode;
use crate::config::EncodeFormat;
use crate::error::{Error, Result};
use crate::threading::{sanitize_threads, Threading};

const DEFAULT_INPUT_BUFFER: usize = 64 * 1024;
const DEFAULT_OUTPUT_BUFFER: usize = 64 * 1024;

/// Configuration builder for XZ compression operations.
#[derive(Debug, Clone)]
pub struct CompressionOptions {
    level: Compression,
    check: IntegrityCheck,
    threads: Threading,
    block_size: Option<NonZeroU64>,
    timeout: Option<Duration>,
    filters: Vec<FilterConfig>,
    format: EncodeFormat,
    lzma1: Option<Lzma1Options>,
    input_buffer_size: NonZeroUsize,
    output_buffer_size: NonZeroUsize,
}

impl Default for CompressionOptions {
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
        }
    }
}

/// Encoder built from [`CompressionOptions`].
pub(crate) enum BuiltEncoder {
    Xz(Encoder),
    Lzma(AloneEncoder),
}

impl BuiltEncoder {
    pub(crate) fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        action: lzma_safe::Action,
    ) -> std::result::Result<(usize, usize), lzma_safe::Error> {
        match self {
            BuiltEncoder::Xz(enc) => enc.process(input, output, action),
            BuiltEncoder::Lzma(enc) => enc.process(input, output, action),
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        match self {
            BuiltEncoder::Xz(enc) => enc.is_finished(),
            BuiltEncoder::Lzma(enc) => enc.is_finished(),
        }
    }
}

impl CompressionOptions {
    /// Sets the compression level (preset).
    ///
    /// Compression levels balance between speed and compression ratio:
    ///
    /// - `Level0-Level3`: Fast compression, lower ratios
    /// - `Level4-Level6`: Balanced (Level6 is default)
    /// - `Level7-Level9`: Slower compression, higher ratios
    #[must_use]
    pub fn with_level(mut self, level: Compression) -> Self {
        self.level = level;
        self
    }

    /// Sets the integrity check algorithm for the compressed stream.
    ///
    /// Available options:
    ///
    /// - `None`: No integrity check (fastest, least secure)
    /// - `Crc32`: Fast CRC32 checksum
    /// - `Crc64`: Balanced CRC64 checksum (default)
    /// - `Sha256`: Cryptographic SHA-256 hash (slowest, most secure)
    #[must_use]
    pub fn with_check(mut self, check: IntegrityCheck) -> Self {
        self.check = check;
        self
    }

    /// Configures the threading strategy for compression.
    ///
    /// - `Threading::Auto`: Automatically choose a safe thread count
    /// - `Threading::Exact(n)`: Use exactly `n` threads (subject to system limits)
    #[must_use]
    pub fn with_threads(mut self, threads: Threading) -> Self {
        self.threads = threads;
        self
    }

    /// Sets a custom block size for multi-threaded compression.
    ///
    /// Block size affects both compression ratio and memory usage:
    ///
    /// - Larger blocks: Better compression ratio, more memory usage
    /// - Smaller blocks: Lower memory usage, potentially worse compression
    ///
    /// If `None` (default), the block size is determined by the compression level.
    /// This setting only applies to multi-threaded compression.
    #[must_use]
    pub fn with_block_size(mut self, block_size: Option<NonZeroU64>) -> Self {
        self.block_size = block_size;
        self
    }

    /// Sets a timeout for multi-threaded compression operations.
    ///
    /// This timeout applies to internal synchronization in the multi-threaded
    /// encoder. It does not limit the total compression time, but rather prevents
    /// indefinite blocking on thread coordination.
    ///
    /// If `None` (default), no timeout is applied. This setting only affects
    /// multi-threaded compression.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets a custom filter chain (advanced usage).
    ///
    /// Filters define the compression algorithm and its parameters. By default,
    /// filters are chosen based on the compression level preset.
    ///
    /// # Warning
    ///
    /// Custom filter chains require deep understanding of LZMA2 compression.
    /// Incorrect filter configurations can result in poor compression ratios
    /// or compression failures. Most users should rely on compression level presets.
    #[must_use]
    pub fn with_filters(mut self, filters: Vec<FilterConfig>) -> Self {
        self.filters = filters;
        self
    }

    /// Selects the output container format.
    #[must_use]
    pub fn with_format(mut self, format: EncodeFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets explicit LZMA1 parameters (only used when format is [`EncodeFormat::Lzma`]).
    ///
    /// If not specified, the LZMA1 options are derived from the selected compression preset.
    #[must_use]
    pub fn with_lzma1_options(mut self, options: Option<Lzma1Options>) -> Self {
        self.lzma1 = options;
        self
    }

    /// Sets the input buffer size for reading source data.
    ///
    /// Larger buffers can improve performance by reducing the number of read
    /// operations, but use more memory. The default (64KB) works well for most cases.
    #[must_use]
    pub fn with_input_buffer_size(mut self, size: NonZeroUsize) -> Self {
        self.input_buffer_size = size;
        self
    }

    /// Sets the output buffer size for compressed data.
    ///
    /// This buffer holds compressed data before it's written to the output.
    /// Larger buffers can improve performance by reducing write operations.
    #[must_use]
    pub fn with_output_buffer_size(mut self, size: NonZeroUsize) -> Self {
        self.output_buffer_size = size;
        self
    }

    pub(crate) fn build_encoder(&self) -> Result<BuiltEncoder> {
        match self.format {
            EncodeFormat::Xz => self.build_xz_encoder().map(BuiltEncoder::Xz),
            EncodeFormat::Lzma => self.build_lzma_encoder().map(BuiltEncoder::Lzma),
        }
    }

    fn build_xz_encoder(&self) -> Result<Encoder> {
        let threads = match sanitize_threads(self.threads) {
            Ok(count) => count.max(1),
            Err(Error::InvalidThreadCount { maximum, .. }) => maximum.max(1),
            Err(other) => return Err(other),
        };
        let stream = Stream::default();

        if threads <= 1
            && self.block_size.is_none()
            && self.timeout.is_none()
            && self.filters.is_empty()
        {
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
            None => Lzma1Options::from_preset(self.level).map_err(Error::from)?,
        };
        AloneEncoder::new(options, Stream::default()).map_err(Error::from)
    }

    pub(crate) fn input_capacity(&self) -> usize {
        self.input_buffer_size.get()
    }

    pub(crate) fn output_capacity(&self) -> usize {
        self.output_buffer_size.get()
    }
}

/// Configuration builder for XZ decompression operations with security-focused defaults.
#[derive(Debug, Clone)]
pub struct DecompressionOptions {
    threads: Threading,
    memlimit: NonZeroU64,
    memlimit_stop: Option<NonZeroU64>,
    flags: DecoderFlags,
    mode: DecodeMode,
    timeout: Option<Duration>,
    input_buffer_size: NonZeroUsize,
    output_buffer_size: NonZeroUsize,
}

impl Default for DecompressionOptions {
    fn default() -> Self {
        Self {
            threads: Threading::Auto,
            memlimit: NonZeroU64::new(256 * 1024 * 1024).unwrap(),
            memlimit_stop: None,
            flags: DecoderFlags::empty(),
            mode: DecodeMode::Auto,
            timeout: None,
            input_buffer_size: NonZeroUsize::new(DEFAULT_INPUT_BUFFER).unwrap(),
            output_buffer_size: NonZeroUsize::new(DEFAULT_OUTPUT_BUFFER).unwrap(),
        }
    }
}

impl DecompressionOptions {
    /// Configures the threading strategy for decompression.
    ///
    /// - `Threading::Auto`: Automatically choose a safe thread count
    /// - `Threading::Exact(n)`: Use exactly `n` threads (subject to format limitations)
    #[must_use]
    pub fn with_threads(mut self, threads: Threading) -> Self {
        self.threads = threads;
        self
    }

    /// Sets the soft memory limit for decompression.
    ///
    /// This is the preferred memory limit. When exceeded during multi-threaded
    /// decompression, the decoder will fall back to using fewer threads or
    /// single-threaded operation to reduce memory usage.
    #[must_use]
    pub fn with_memlimit(mut self, limit: NonZeroU64) -> Self {
        self.memlimit = limit;
        self
    }

    /// Sets the hard memory limit for decompression.
    ///
    /// If memory usage exceeds this limit, decompression will fail immediately
    /// with an error. This provides a hard boundary against memory exhaustion attacks.
    ///
    /// If `None` (default), the hard limit equals the soft limit. The hard limit
    /// must be greater than or equal to the soft limit.
    #[must_use]
    pub fn with_memlimit_stop(mut self, limit: Option<NonZeroU64>) -> Self {
        self.memlimit_stop = limit;
        self
    }

    /// Sets decoder flags to control parsing behavior.
    ///
    /// Available flags:
    ///
    /// - `CONCATENATED`: Allow processing of concatenated streams
    /// - `IGNORE_CHECK`: Skip integrity check verification (not recommended)
    ///
    /// The default (empty flags) provides the most secure and strict parsing.
    ///
    /// # Warning
    ///
    /// Using `IGNORE_CHECK` disables integrity verification and should only be used
    /// when you have other means of ensuring data integrity.
    #[must_use]
    pub fn with_flags(mut self, flags: DecoderFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Selects the decompression format and decoder type.
    ///
    /// Available modes:
    ///
    /// - `DecodeMode::Auto`: Automatically detect XZ or LZMA format (single-threaded only)
    /// - `DecodeMode::Xz`: Force XZ format parsing (supports multi-threading)
    /// - `DecodeMode::Lzma`: Force LZMA format parsing (single-threaded only)
    #[must_use]
    pub fn with_mode(mut self, mode: DecodeMode) -> Self {
        self.mode = mode;
        self
    }

    /// Sets a timeout for multi-threaded decompression operations.
    ///
    /// This timeout applies to internal thread coordination in the multi-threaded
    /// decoder. It does not limit the total decompression time, but prevents
    /// indefinite blocking on thread synchronization.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the input buffer size for reading compressed data.
    ///
    /// Larger buffers can improve performance by reducing the number of read
    /// operations, but use more memory. The default (64KB) works well for most cases.
    #[must_use]
    pub fn with_input_buffer_size(mut self, size: NonZeroUsize) -> Self {
        self.input_buffer_size = size;
        self
    }

    /// Sets the output buffer size for decompressed data.
    ///
    /// This buffer holds decompressed data before it's written to the output.
    /// Larger buffers can improve performance by reducing write operations.
    #[must_use]
    pub fn with_output_buffer_size(mut self, size: NonZeroUsize) -> Self {
        self.output_buffer_size = size;
        self
    }

    pub(crate) fn build_decoder(&self) -> Result<Decoder> {
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

                Decoder::new_auto(memlimit, self.flags, stream).map_err(Error::from)
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

                Decoder::new_mt(options, stream).map_err(Error::from)
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

                Decoder::new_alone(memlimit, stream).map_err(Error::from)
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
}

/// Converts a `Duration` to a timeout value in milliseconds for the LZMA library.
///
/// # Parameters
///
/// * `duration` - The timeout duration to convert
///
/// # Returns
///
/// The timeout in milliseconds as a `u32`.
fn duration_to_timeout(duration: Duration) -> u32 {
    duration.as_millis().try_into().unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use crate::error::Error;

    use super::*;

    /// Test that [`CompressionOptions`] has expected defaults.
    #[test]
    fn compression_options_defaults() {
        let options = CompressionOptions::default();
        assert_eq!(options.input_capacity(), DEFAULT_INPUT_BUFFER);
        assert_eq!(options.output_capacity(), DEFAULT_OUTPUT_BUFFER);
    }

    /// Test that custom buffer sizes are reflected in helper accessors.
    #[test]
    fn compression_buffer_sizes_follow_configuration() {
        let input_size = NonZeroUsize::new(8 * 1024).unwrap();
        let output_size = NonZeroUsize::new(16 * 1024).unwrap();
        let options = CompressionOptions::default()
            .with_input_buffer_size(input_size)
            .with_output_buffer_size(output_size);

        assert_eq!(options.input_capacity(), input_size.get());
        assert_eq!(options.output_capacity(), output_size.get());
    }

    /// Test that compression options builder methods work correctly.
    #[test]
    fn compression_options_builder_pattern() {
        use lzma_safe::encoder::options::{Compression, IntegrityCheck};
        use std::time::Duration;

        let options = CompressionOptions::default()
            .with_level(Compression::Level9)
            .with_check(IntegrityCheck::Sha256)
            .with_threads(Threading::Exact(4))
            .with_block_size(Some(NonZeroU64::new(1024 * 1024).unwrap()))
            .with_timeout(Some(Duration::from_secs(30)))
            .with_filters(vec![]);

        // Test that the builder pattern returns Self and allows chaining
        assert_eq!(options.level, Compression::Level9);
        assert_eq!(options.check, IntegrityCheck::Sha256);
        assert_eq!(options.threads, Threading::Exact(4));
        assert_eq!(
            options.block_size,
            Some(NonZeroU64::new(1024 * 1024).unwrap())
        );
        assert_eq!(options.timeout, Some(Duration::from_secs(30)));
        assert!(options.filters.is_empty());
    }

    /// Test that zero-thread requests are handled gracefully in compression.
    #[test]
    fn compression_zero_thread_request_is_clamped() {
        let options = CompressionOptions::default().with_threads(Threading::Exact(0));
        options
            .build_encoder()
            .expect("sanitized thread count should construct encoder");
    }

    /// Test that single-threaded compression works with various configurations.
    #[test]
    fn compression_single_threaded_builds_successfully() {
        let options = CompressionOptions::default().with_threads(Threading::Exact(1));

        options
            .build_encoder()
            .expect("single-threaded encoder should build successfully");
    }

    /// Test that auto-threading compression builds successfully.
    #[test]
    fn compression_auto_threading_builds_successfully() {
        let options = CompressionOptions::default().with_threads(Threading::Auto);

        options
            .build_encoder()
            .expect("auto-threaded encoder should build successfully");
    }

    /// Test that [`DecompressionOptions`] has expected defaults.
    #[test]
    fn decompression_options_defaults() {
        let options = DecompressionOptions::default();
        assert_eq!(options.input_capacity(), DEFAULT_INPUT_BUFFER);
        assert_eq!(options.output_capacity(), DEFAULT_OUTPUT_BUFFER);
        assert_eq!(options.memlimit.get(), 256 * 1024 * 1024); // 256MB default
        assert_eq!(options.threads, Threading::Auto);
        assert_eq!(options.mode, DecodeMode::Auto);
        assert!(options.flags.is_empty());
    }

    /// Test that decompression buffer sizes follow configuration.
    #[test]
    fn decompression_buffer_sizes_follow_configuration() {
        let input_size = NonZeroUsize::new(32 * 1024).unwrap();
        let output_size = NonZeroUsize::new(128 * 1024).unwrap();
        let options = DecompressionOptions::default()
            .with_input_buffer_size(input_size)
            .with_output_buffer_size(output_size);

        assert_eq!(options.input_capacity(), input_size.get());
        assert_eq!(options.output_capacity(), output_size.get());
    }

    /// Test that decompression options builder pattern works.
    #[test]
    fn decompression_options_builder_pattern() {
        use lzma_safe::decoder::options::Flags;
        use std::time::Duration;

        let memlimit = NonZeroU64::new(128 * 1024 * 1024).unwrap();
        let memlimit_stop = NonZeroU64::new(256 * 1024 * 1024).unwrap();

        let options = DecompressionOptions::default()
            .with_threads(Threading::Exact(2))
            .with_memlimit(memlimit)
            .with_memlimit_stop(Some(memlimit_stop))
            .with_flags(Flags::CONCATENATED)
            .with_mode(DecodeMode::Xz)
            .with_timeout(Some(Duration::from_secs(10)));

        assert_eq!(options.threads, Threading::Exact(2));
        assert_eq!(options.memlimit, memlimit);
        assert_eq!(options.memlimit_stop, Some(memlimit_stop));
        assert!(options.flags.is_concatenated());
        assert_eq!(options.mode, DecodeMode::Xz);
        assert_eq!(options.timeout, Some(Duration::from_secs(10)));
    }

    /// Test that `memlimit_stop` validation rejects unsafe configurations.
    #[test]
    fn decompression_memlimit_stop_must_exceed_soft_limit() {
        let options = DecompressionOptions::default()
            .with_memlimit(NonZeroU64::new(2_048).unwrap())
            .with_memlimit_stop(Some(NonZeroU64::new(1_024).unwrap()));

        assert!(matches!(
            options.build_decoder(),
            Err(Error::InvalidOption(_))
        ));
    }

    /// Test that `memlimit_stop` equal to `memlimit` is allowed.
    #[test]
    fn decompression_memlimit_stop_equal_to_soft_limit_allowed() {
        let limit = NonZeroU64::new(64 * 1024 * 1024).unwrap();
        let options = DecompressionOptions::default()
            .with_memlimit(limit)
            .with_memlimit_stop(Some(limit));

        // Should not fail validation
        let result = options.build_decoder();
        assert!(result.is_ok() || matches!(result, Err(Error::ThreadingUnsupported { .. })));
    }

    /// Test that LZMA mode rejects multi-threading.
    #[test]
    fn lzma_mode_rejects_multi_threading() {
        let options = DecompressionOptions::default()
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

    /// Test that Auto mode rejects explicit multi-threading.
    #[test]
    fn auto_mode_rejects_explicit_multi_threading() {
        let options = DecompressionOptions::default()
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

    /// Test that Auto mode accepts single-threading.
    #[test]
    fn auto_mode_accepts_single_threading() {
        let options = DecompressionOptions::default()
            .with_mode(DecodeMode::Auto)
            .with_threads(Threading::Exact(1));

        options
            .build_decoder()
            .expect("Auto mode should accept single threading");
    }

    /// Test that XZ mode accepts multi-threading.
    #[test]
    fn xz_mode_accepts_multi_threading() {
        let options = DecompressionOptions::default()
            .with_mode(DecodeMode::Xz)
            .with_threads(Threading::Exact(2));

        options
            .build_decoder()
            .expect("XZ mode should accept multi-threading");
    }

    /// Test that timeout conversion handles various durations correctly.
    #[test]
    fn timeout_conversion_handles_normal_durations() {
        assert_eq!(duration_to_timeout(Duration::from_millis(0)), 0);
        assert_eq!(duration_to_timeout(Duration::from_millis(1)), 1);
        assert_eq!(duration_to_timeout(Duration::from_millis(1000)), 1000);
        assert_eq!(duration_to_timeout(Duration::from_secs(1)), 1000);
        assert_eq!(duration_to_timeout(Duration::from_secs(60)), 60000);
    }

    /// Test that timeout conversion saturates at `u32::MAX`.
    #[test]
    fn timeout_conversion_saturates() {
        let overflowing = Duration::from_millis(u64::from(u32::MAX) + 5);
        assert_eq!(duration_to_timeout(overflowing), u32::MAX);

        let exact_max = Duration::from_millis(u64::from(u32::MAX));
        assert_eq!(duration_to_timeout(exact_max), u32::MAX);

        let just_under_max = Duration::from_millis(u64::from(u32::MAX) - 1);
        assert_eq!(duration_to_timeout(just_under_max), u32::MAX - 1);
    }

    /// Test that timeout conversion handles edge cases.
    #[test]
    fn timeout_conversion_edge_cases() {
        // Test maximum safe Duration that fits in u32 milliseconds
        let max_safe = Duration::from_millis(u64::from(u32::MAX));
        assert_eq!(duration_to_timeout(max_safe), u32::MAX);

        // Test very large Duration
        let huge = Duration::from_secs(u64::MAX);
        assert_eq!(duration_to_timeout(huge), u32::MAX);
    }

    /// Test that Default trait works for both option types.
    #[test]
    fn options_default_trait_works() {
        let compression_default = CompressionOptions::default();
        let compression_new = CompressionOptions::default();

        // Both should have the same buffer sizes
        assert_eq!(
            compression_default.input_capacity(),
            compression_new.input_capacity()
        );
        assert_eq!(
            compression_default.output_capacity(),
            compression_new.output_capacity()
        );

        let decompression_default = DecompressionOptions::default();
        let decompression_new = DecompressionOptions::default();

        assert_eq!(
            decompression_default.input_capacity(),
            decompression_new.input_capacity()
        );
        assert_eq!(
            decompression_default.output_capacity(),
            decompression_new.output_capacity()
        );
    }

    /// Test that Clone trait works correctly for options.
    #[test]
    fn options_clone_works() {
        let original = CompressionOptions::default()
            .with_level(lzma_safe::encoder::options::Compression::Level9);
        let cloned = original.clone();

        assert_eq!(original.level, cloned.level);
        assert_eq!(original.input_capacity(), cloned.input_capacity());

        let original_decomp = DecompressionOptions::default()
            .with_memlimit(NonZeroU64::new(128 * 1024 * 1024).unwrap());
        let cloned_decomp = original_decomp.clone();

        assert_eq!(original_decomp.memlimit, cloned_decomp.memlimit);
        assert_eq!(
            original_decomp.input_capacity(),
            cloned_decomp.input_capacity()
        );
    }
}
