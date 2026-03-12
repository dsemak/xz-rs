//! Shared configuration primitives and types for XZ stream processing.

/// Decoder format selection and processing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeMode {
    /// Automatically detect and process both XZ and LZMA format streams.
    ///
    /// This mode provides maximum compatibility by automatically detecting
    /// the input format and using the appropriate decoder. However, it's
    /// limited to single-threaded operation for security and simplicity.
    ///
    /// **Threading**: Single-threaded only
    /// **Formats**: XZ (.xz), LZMA (.lzma), and lzip (.lz)
    /// **Use case**: Processing streams of unknown format
    Auto,

    /// Process XZ format streams exclusively.
    ///
    /// This mode only accepts XZ container format and provides the best
    /// performance through multi-threaded decompression support. Use this
    /// when you know the input is XZ format and want maximum performance.
    ///
    /// **Threading**: Single-threaded and multi-threaded
    /// **Formats**: XZ (.xz) only
    /// **Use case**: High-performance XZ decompression
    Xz,

    /// Process legacy LZMA format streams exclusively.
    ///
    /// This mode only accepts the legacy LZMA1 format (not LZMA2 used in XZ).
    /// It's primarily for compatibility with older compressed data.
    ///
    /// **Threading**: Single-threaded only
    /// **Formats**: LZMA (.lzma) only
    /// **Use case**: Legacy LZMA stream compatibility
    Lzma,

    /// Process raw LZMA1 filter streams with explicitly supplied filter options.
    ///
    /// This mode has no container metadata, so the caller must configure the filter chain
    /// out-of-band and threading is not supported.
    Raw,
}

/// Policy controlling how auto-detect decompression handles unknown input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownInputPolicy {
    /// Return an error when the input doesn't look like a supported container.
    Error,
    /// Copy the original input to the output unchanged.
    Passthrough,
}

/// High-level result status for a decompression operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecompressionStatus {
    /// Input was decoded as a supported compressed stream.
    Decompressed,
    /// Input was copied to the output unchanged.
    Passthrough,
}

/// Encoder container format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeFormat {
    /// XZ container format (default).
    Xz,
    /// Legacy `.lzma` (`LZMA_Alone`) container format.
    Lzma,
    /// Raw LZMA1 filter stream without any container wrapper.
    Raw,
}

/// Statistical summary of completed stream processing operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSummary {
    /// Total number of bytes read from the input source.
    pub bytes_read: u64,

    /// Total number of bytes written to the output destination.
    pub bytes_written: u64,
}

impl StreamSummary {
    /// Creates a new stream summary with the specified byte counts.
    ///
    /// This is primarily used internally by the compression and decompression
    /// pipelines to create summary statistics after processing completes.
    ///
    /// # Parameters
    ///
    /// * `bytes_read` - Total bytes consumed from the input stream
    /// * `bytes_written` - Total bytes produced to the output stream
    ///
    /// # Returns
    ///
    /// A new [`StreamSummary`] instance with the specified byte counts.
    pub(crate) const fn new(bytes_read: u64, bytes_written: u64) -> Self {
        Self {
            bytes_read,
            bytes_written,
        }
    }

    /// Calculates the compression ratio for this stream summary.
    ///
    /// # Returns
    ///
    /// The compression ratio as an `f64`. A value less than 1.0 indicates
    /// compression occurred, while a value greater than 1.0 indicates expansion.
    #[allow(clippy::cast_precision_loss)]
    pub fn compression_ratio(&self) -> f64 {
        if self.bytes_read == 0 {
            if self.bytes_written == 0 {
                0.0
            } else {
                f64::INFINITY
            }
        } else {
            self.bytes_written as f64 / self.bytes_read as f64
        }
    }

    /// Calculates the space saved percentage for compression operations.
    ///
    /// # Returns
    ///
    /// The space saved as a percentage (0.0 to 100.0). Positive values indicate
    /// space was saved through compression. Negative values indicate the output
    /// was larger than the input (expansion occurred).
    pub fn space_saved_percent(&self) -> f64 {
        if self.bytes_read == 0 {
            0.0
        } else {
            let ratio = self.compression_ratio();
            (1.0 - ratio) * 100.0
        }
    }
}

/// Result of a completed decompression operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecompressionOutcome {
    /// Total number of bytes read from the input source.
    pub bytes_read: u64,

    /// Total number of bytes written to the output destination.
    pub bytes_written: u64,

    /// Whether the pipeline decoded compressed data or passed the input through.
    pub status: DecompressionStatus,

    /// Integrity check ID from the XZ stream header when it isn't supported by liblzma.
    pub unsupported_check_id: Option<u32>,
}

impl DecompressionOutcome {
    /// Creates a new decompression outcome from a stream summary and metadata.
    pub(crate) const fn new(
        summary: StreamSummary,
        status: DecompressionStatus,
        unsupported_check_id: Option<u32>,
    ) -> Self {
        Self {
            bytes_read: summary.bytes_read,
            bytes_written: summary.bytes_written,
            status,
            unsupported_check_id,
        }
    }

    /// Calculates the compression ratio for this outcome.
    #[allow(clippy::cast_precision_loss)]
    pub fn compression_ratio(&self) -> f64 {
        if self.bytes_read == 0 {
            if self.bytes_written == 0 {
                0.0
            } else {
                f64::INFINITY
            }
        } else {
            self.bytes_written as f64 / self.bytes_read as f64
        }
    }

    /// Calculates the space saved percentage for compression operations.
    pub fn space_saved_percent(&self) -> f64 {
        if self.bytes_read == 0 {
            0.0
        } else {
            let ratio = self.compression_ratio();
            (1.0 - ratio) * 100.0
        }
    }
}
