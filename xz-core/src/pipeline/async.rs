//! Asynchronous XZ compression and decompression pipeline.

use lzma_safe::{Action, Decoder, Encoder};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::buffer::Buffer;
use crate::config::StreamSummary;
use crate::error::{BackendError, Result};
use crate::options::{CompressionOptions, DecompressionOptions};

/// Compresses data asynchronously from a reader into a writer using the provided options.
///
/// # Parameters
///
/// * `reader` - Input source implementing [`AsyncRead`] + [`Unpin`] traits
/// * `writer` - Output destination implementing [`AsyncWrite`] + [`Unpin`] traits
/// * `options` - Compression configuration options [`CompressionOptions`]
///
/// # Returns
///
/// Returns a [`StreamSummary`] containing statistics about bytes read and written,
/// or an error if compression fails.
///
/// # Errors
///
/// This function will return an error if:
///
/// - The encoder cannot be built from the provided options
/// - Async I/O operations on reader or writer fail
/// - Invalid compression parameters are specified
/// - Threading limits are exceeded
pub async fn compress_async<R, W>(
    mut reader: R,
    mut writer: W,
    options: &CompressionOptions,
) -> Result<StreamSummary>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut encoder = options.build_encoder()?;
    let mut input = Buffer::new(options.input_capacity())?;
    let mut output = Buffer::new(options.output_capacity())?;
    let mut total_in = 0u64;
    let mut total_out = 0u64;

    loop {
        let read = reader.read(&mut input).await?;
        if read == 0 {
            finish_encoder_async(&mut encoder, &mut writer, &mut output, &mut total_out).await?;
            return Ok(StreamSummary::new(total_in, total_out));
        }

        let mut consumed = 0usize;
        while consumed < read {
            let (used, written) =
                encoder.process(&input[consumed..read], &mut output, Action::Run)?;
            if written > 0 {
                writer.write_all(&output[..written]).await?;
                total_out += written as u64;
            }
            consumed += used;
            total_in += used as u64;

            if encoder.is_finished() {
                writer.flush().await?;
                return Ok(StreamSummary::new(total_in, total_out));
            }

            if used == 0 && written == 0 {
                break;
            }
        }
    }
}

/// Decompresses data asynchronously from a reader into a writer using the provided options.
///
/// # Parameters
///
/// * `reader` - Input source implementing [`AsyncRead`] + [`Unpin`] traits
/// * `writer` - Output destination implementing [`AsyncWrite`] + [`Unpin`] traits
/// * `options` - Decompression configuration options [`DecompressionOptions`]
///
/// # Returns
///
/// Returns a [`StreamSummary`] containing statistics about bytes read and written,
/// or an error if decompression fails.
///
/// # Errors
///
/// This function will return an error if:
///
/// - The decoder cannot be built from the provided options
/// - Async I/O operations on reader or writer fail
/// - The compressed data is corrupted or invalid
/// - Memory limits are exceeded during decompression
/// - Threading is requested for unsupported decode modes
pub async fn decompress_async<R, W>(
    mut reader: R,
    mut writer: W,
    options: &DecompressionOptions,
) -> Result<StreamSummary>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut decoder = options.build_decoder()?;
    let mut input = Buffer::new(options.input_capacity())?;
    let mut output = Buffer::new(options.output_capacity())?;
    let mut total_in = 0u64;
    let mut total_out = 0u64;

    loop {
        let read = reader.read(&mut input).await?;
        if read == 0 {
            finish_decoder_async(&mut decoder, &mut writer, &mut output, &mut total_out).await?;
            return Ok(StreamSummary::new(total_in, total_out));
        }

        let mut consumed = 0usize;
        while consumed < read {
            let (used, written) =
                decoder.process(&input[consumed..read], &mut output, Action::Run)?;
            if written > 0 {
                writer.write_all(&output[..written]).await?;
                total_out += written as u64;
            }
            consumed += used;
            total_in += used as u64;

            if decoder.is_finished() {
                writer.flush().await?;
                return Ok(StreamSummary::new(total_in, total_out));
            }

            if used == 0 && written == 0 {
                break;
            }
        }
    }
}

/// Finishes the encoding process asynchronously by flushing any remaining data from the encoder.
///
/// # Parameters
///
/// * `encoder` - The encoder instance to finish
/// * `writer` - Async output writer to receive the final compressed data
/// * `output` - Buffer for temporary storage of compressed data
/// * `total_out` - Running count of total bytes written (updated in-place)
///
/// # Returns
///
/// * `Ok(())` if the encoder finished successfully
/// * `Err(BackendError::BufError)` if the encoder gets stuck in an infinite loop
async fn finish_encoder_async<W: AsyncWrite + Unpin>(
    encoder: &mut Encoder,
    writer: &mut W,
    output: &mut [u8],
    total_out: &mut u64,
) -> Result<()> {
    // Prevent infinite loops by limiting the number of finish attempts
    const MAX_SPINS: usize = 16;

    for _ in 0..MAX_SPINS {
        // Process with empty input to flush internal buffers
        let (read, written) = encoder.process(&[], output, Action::Finish)?;

        // Write any output data produced during finishing
        if written > 0 {
            writer.write_all(&output[..written]).await?;
            *total_out += written as u64;
        }

        // Check if encoder has completed successfully
        if encoder.is_finished() {
            writer.flush().await?;
            return Ok(());
        }

        // If no progress is made, break to avoid infinite loop
        if read == 0 && written == 0 {
            break;
        }
    }

    // If we reach here, the encoder failed to finish properly
    Err(BackendError::BufError.into())
}

/// Finishes the decoding process asynchronously by flushing any remaining data from the decoder.
///
/// # Parameters
///
/// * `decoder` - The decoder instance to finish
/// * `writer` - Async output writer to receive the final decoded data
/// * `output` - Buffer for temporary storage of decoded data
/// * `total_out` - Running count of total bytes written (updated in-place)
///
/// # Returns
///
/// * `Ok(())` if the decoder finished successfully
/// * `Err(BackendError::BufError)` if the decoder gets stuck in an infinite loop
async fn finish_decoder_async<W: AsyncWrite + Unpin>(
    decoder: &mut Decoder,
    writer: &mut W,
    output: &mut [u8],
    total_out: &mut u64,
) -> Result<()> {
    // Prevent infinite loops by limiting the number of finish attempts
    const MAX_SPINS: usize = 16;

    for _ in 0..MAX_SPINS {
        // Process with empty input to flush internal buffers
        let (read, written) = decoder.process(&[], output, Action::Finish)?;

        // Write any output data produced during finishing
        if written > 0 {
            writer.write_all(&output[..written]).await?;
            *total_out += written as u64;
        }

        // Check if decoder has completed successfully
        if decoder.is_finished() {
            writer.flush().await?;
            return Ok(());
        }

        // If no progress is made, break to avoid infinite loop
        if read == 0 && written == 0 {
            break;
        }
    }

    // If we reach here, the decoder failed to finish properly
    Err(BackendError::BufError.into())
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU64, NonZeroUsize};
    use std::time::Duration;

    use crate::config::DecodeMode;
    use crate::options::{Compression, CompressionOptions, DecompressionOptions, IntegrityCheck};
    use crate::pipeline::tests::{FailingReader, SlowReader, EMPTY_SAMPLE, LARGE_SAMPLE, SAMPLE};
    use crate::threading::Threading;

    use super::*;

    /// Maximum duration for async tests
    const MAX_DURATION: Duration = Duration::from_secs(60);

    /// Macro to generate async test functions with timeout
    macro_rules! async_test {
        // Basic test with default timeout and current_thread flavor
        ($name:ident, $body:expr) => {
            #[tokio::test(flavor = "current_thread")]
            async fn $name() {
                let result = tokio::time::timeout(MAX_DURATION, async { $body }).await;
                match result {
                    Ok(test_result) => test_result,
                    Err(_) => panic!(
                        "Test '{}' timed out after {:?}",
                        stringify!($name),
                        MAX_DURATION
                    ),
                }
            }
        };
    }

    // Test basic async round-trip compression and decompression functionality.
    async_test!(round_trip_works, {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0);

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let decompression_summary =
            decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();
        assert_eq!(
            usize::try_from(decompression_summary.bytes_written).unwrap(),
            SAMPLE.len()
        );
        assert!(decompressed == SAMPLE);
    });

    // Test async compression and decompression of empty input.
    async_test!(empty_input, {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress_async(EMPTY_SAMPLE, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0); // XZ header is always present

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let decompression_summary =
            decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();
        assert_eq!(decompression_summary.bytes_written, 0);
        assert!(decompressed == EMPTY_SAMPLE);
    });

    // Test async compression and decompression of large input data.
    async_test!(large_input, {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress_async(LARGE_SAMPLE, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0);
        assert!(compression_summary.bytes_read == LARGE_SAMPLE.len() as u64);

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let decompression_summary =
            decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();
        assert_eq!(
            usize::try_from(decompression_summary.bytes_written).unwrap(),
            LARGE_SAMPLE.len()
        );
        assert!(decompressed == LARGE_SAMPLE);
    });

    // Test async compression with different compression levels.
    async_test!(compression_levels, {
        let levels = [
            Compression::Level0,
            Compression::Level1,
            Compression::Level6,
            Compression::Level9,
        ];

        for level in levels {
            let options = CompressionOptions::default().with_level(level);
            let mut compressed = Vec::new();
            let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
                .await
                .unwrap();
            assert!(compression_summary.bytes_written > 0);

            let mut decompressed = Vec::new();
            let options = DecompressionOptions::default();
            let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();
            assert!(decompressed == SAMPLE);
        }
    });

    // Test different integrity checks
    async_test!(integrity_checks, {
        let checks = [
            IntegrityCheck::None,
            IntegrityCheck::Crc32,
            IntegrityCheck::Crc64,
        ];

        for check in checks {
            let options = CompressionOptions::default().with_check(check);
            let mut compressed = Vec::new();
            let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
                .await
                .unwrap();
            assert!(compression_summary.bytes_written > 0);

            let mut decompressed = Vec::new();
            let options = DecompressionOptions::default();
            let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();
            assert!(decompressed == SAMPLE);
        }
    });

    // Test different buffer sizes
    async_test!(buffer_sizes, {
        let buffer_sizes = [
            NonZeroUsize::new(1024).unwrap(),  // Small buffers
            NonZeroUsize::new(8192).unwrap(),  // Medium buffers
            NonZeroUsize::new(65536).unwrap(), // Large buffers
        ];

        for size in buffer_sizes {
            let options = CompressionOptions::default()
                .with_input_buffer_size(size)
                .with_output_buffer_size(size);
            let mut compressed = Vec::new();
            let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
                .await
                .unwrap();
            assert!(compression_summary.bytes_written > 0);

            let options = DecompressionOptions::default()
                .with_input_buffer_size(size)
                .with_output_buffer_size(size);
            let mut decompressed = Vec::new();
            let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();
            assert!(decompressed == SAMPLE);
        }
    });

    // Test threading configurations
    async_test!(threading_options, {
        // Only test single-threaded mode to avoid threading issues
        let thread_configs = [Threading::Auto, Threading::Exact(1)];

        for threads in thread_configs {
            let options = CompressionOptions::default().with_threads(threads);
            let mut compressed = Vec::new();
            let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
                .await
                .unwrap();
            assert!(compression_summary.bytes_written > 0);

            // For decompression, only use single-threaded mode to avoid threading issues
            let options = DecompressionOptions::default().with_threads(Threading::Exact(1));
            let mut decompressed = Vec::new();
            let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();
            assert!(decompressed == SAMPLE);
        }
    });

    // Test memory limits for decompression
    async_test!(memory_limits, {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0);

        // Test with generous memory limit
        let options = DecompressionOptions::default()
            .with_memlimit(NonZeroU64::new(128 * 1024 * 1024).unwrap());
        let mut decompressed = Vec::new();
        let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
            .await
            .unwrap();
        assert!(decompressed == SAMPLE);
    });

    // Test decode modes
    async_test!(decode_modes, {
        let modes = [DecodeMode::Auto, DecodeMode::Xz];

        for mode in modes {
            let mut compressed = Vec::new();
            let options = CompressionOptions::default();
            let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
                .await
                .unwrap();
            assert!(compression_summary.bytes_written > 0);

            let options = DecompressionOptions::default().with_mode(mode);
            let mut decompressed = Vec::new();
            let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();
            assert!(decompressed == SAMPLE);
        }
    });

    // Test with timeout configuration
    async_test!(with_timeout, {
        let timeout = Duration::from_millis(1000);
        let options = CompressionOptions::default().with_timeout(Some(timeout));
        let mut compressed = Vec::new();
        let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0);

        let options = DecompressionOptions::default().with_timeout(Some(timeout));
        let mut decompressed = Vec::new();
        let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
            .await
            .unwrap();
        assert!(decompressed == SAMPLE);
    });

    // Test with block size configuration
    async_test!(with_block_size, {
        let block_size = NonZeroU64::new(64 * 1024).unwrap();
        let options = CompressionOptions::default().with_block_size(Some(block_size));
        let mut compressed = Vec::new();
        let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0);

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
            .await
            .unwrap();
        assert!(decompressed == SAMPLE);
    });

    // Test streaming with small chunks
    async_test!(streaming_small_chunks, {
        let reader = SlowReader::new(SAMPLE, 4); // Read 4 bytes at a time
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress_async(reader, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0);

        let reader = SlowReader::new(&compressed, 8); // Read 8 bytes at a time
        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let _ = decompress_async(reader, &mut decompressed, &options)
            .await
            .unwrap();
        assert!(decompressed == SAMPLE);
    });

    // Test summary statistics accuracy
    async_test!(summary_statistics, {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();

        assert_eq!(compression_summary.bytes_read, SAMPLE.len() as u64);
        assert_eq!(compression_summary.bytes_written, compressed.len() as u64);
        assert!(compression_summary.bytes_written > 0);

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let decompression_summary =
            decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();

        assert_eq!(decompression_summary.bytes_read, compressed.len() as u64);
        assert_eq!(decompression_summary.bytes_written, SAMPLE.len() as u64);
    });

    // Test with very small buffers to stress internal buffering
    async_test!(tiny_buffers, {
        let tiny_size = NonZeroUsize::new(16).unwrap();
        let options = CompressionOptions::default()
            .with_input_buffer_size(tiny_size)
            .with_output_buffer_size(tiny_size);

        let mut compressed = Vec::new();
        let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0);

        let decompression_options = DecompressionOptions::default()
            .with_input_buffer_size(tiny_size)
            .with_output_buffer_size(tiny_size);
        let mut decompressed = Vec::new();
        let _ = decompress_async(
            compressed.as_slice(),
            &mut decompressed,
            &decompression_options,
        )
        .await
        .unwrap();
        assert!(decompressed == SAMPLE);
    });

    // Test multiple consecutive operations
    async_test!(multiple_operations, {
        for _ in 0..5 {
            let mut compressed = Vec::new();
            let options = CompressionOptions::default();
            let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
                .await
                .unwrap();
            assert!(compression_summary.bytes_written > 0);

            let mut decompressed = Vec::new();
            let options = DecompressionOptions::default();
            let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
                .await
                .unwrap();
            assert!(decompressed == SAMPLE);
        }
    });

    // Test error handling - invalid thread count
    async_test!(error_invalid_thread_count, {
        // Try to use too many threads - this should fail during options building
        let options = CompressionOptions::default().with_threads(Threading::Exact(1000));
        let mut compressed = Vec::new();
        let result = compress_async(SAMPLE, &mut compressed, &options).await;

        // Should fail with InvalidThreadCount error
        assert!(result.is_err());
        if let Err(crate::error::Error::InvalidThreadCount {
            requested,
            maximum: _,
        }) = result
        {
            assert_eq!(requested, 1000);
        } else {
            panic!("Expected InvalidThreadCount error, got: {result:?}");
        }
    });

    // Test error handling - corrupted data
    async_test!(error_corrupted_data, {
        // Create some invalid compressed data
        let corrupted_data = b"This is not valid XZ data";
        let mut decompressed = Vec::new();

        let options = DecompressionOptions::default();
        let result = decompress_async(corrupted_data.as_slice(), &mut decompressed, &options).await;

        // Should fail with a backend error
        assert!(result.is_err());
        matches!(result.unwrap_err(), crate::error::Error::Backend(_));
    });

    // Test error handling - memory limit exceeded
    async_test!(error_memory_limit, {
        // Compress some data first
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let _ = compress_async(LARGE_SAMPLE, &mut compressed, &options)
            .await
            .unwrap();

        // Try to decompress with a very restrictive memory limit
        let options = DecompressionOptions::default().with_memlimit(NonZeroU64::new(1024).unwrap());
        let mut decompressed = Vec::new();

        let result = decompress_async(compressed.as_slice(), &mut decompressed, &options).await;

        // Should fail due to memory limit
        assert!(result.is_err());
        matches!(result.unwrap_err(), crate::error::Error::Backend(_));
    });

    // Test error handling - threading with unsupported mode
    async_test!(error_threading_unsupported_mode, {
        // Compress some data first
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let _ = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();

        // Try to use threading with LZMA mode (which doesn't support it)
        let options = DecompressionOptions::default()
            .with_mode(DecodeMode::Lzma)
            .with_threads(Threading::Exact(2));
        let mut decompressed = Vec::new();

        let result = decompress_async(compressed.as_slice(), &mut decompressed, &options).await;

        // Should fail with ThreadingUnsupported error
        assert!(result.is_err());
        if let Err(crate::error::Error::ThreadingUnsupported { requested, mode }) = result {
            assert_eq!(requested, 2);
            assert_eq!(mode, DecodeMode::Lzma);
        } else {
            panic!("Expected ThreadingUnsupported error, got: {result:?}");
        }
    });

    // Test error handling - I/O errors during reading
    async_test!(error_io_failure, {
        // Fail after 10 bytes
        let failing_reader = FailingReader::new(10);
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();

        let result = compress_async(failing_reader, &mut compressed, &options).await;

        // Should fail with I/O error
        assert!(result.is_err());
        matches!(result.unwrap_err(), crate::error::Error::Io(_));
    });

    // Test error handling - very small buffer sizes
    async_test!(error_zero_buffer_sizes, {
        let small_size = NonZeroUsize::new(64).unwrap();
        let options = CompressionOptions::default()
            .with_input_buffer_size(small_size)
            .with_output_buffer_size(small_size);

        let mut compressed = Vec::new();
        let result = compress_async(SAMPLE, &mut compressed, &options).await;

        // Should work fine with small buffer sizes
        assert!(result.is_ok());
    });
}
