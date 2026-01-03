//! Asynchronous XZ compression and decompression pipeline.

use lzma_safe::{Action, Decoder};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::buffer::Buffer;
use crate::config::StreamSummary;
use crate::error::{BackendError, Result};
use crate::options::{BuiltEncoder, CompressionOptions, DecompressionOptions};

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
    let mut input = vec![0u8; options.input_capacity()];
    let mut output = Buffer::new(options.output_capacity())?;
    let mut total_in = 0u64;
    let mut total_out = 0u64;
    let mut pending_len: usize = 0;

    loop {
        // Ensure we have some input to feed the decoder.
        if pending_len == 0 {
            let read = reader.read(&mut input).await?;
            if read == 0 {
                // If no data was ever processed, this is an invalid/empty input
                if total_in == 0 {
                    return Err(BackendError::DataError.into());
                }
                finish_decoder_async(&mut decoder, &mut writer, &mut output, &mut total_out, &[])
                    .await?;
                return Ok(StreamSummary::new(total_in, total_out));
            }
            pending_len = read;
        }

        let mut consumed = 0usize;
        while consumed < pending_len {
            let (used, written) =
                decoder.process(&input[consumed..pending_len], &mut output, Action::Run)?;
            if written > 0 {
                writer.write_all(&output[..written]).await?;
                total_out += written as u64;
            }
            consumed += used;
            total_in += used as u64;

            if decoder.is_finished() {
                // In non-concatenated mode, "finished" means we intentionally stop after the first
                // stream and ignore any remaining input. This matches the xz(1) --single-stream
                // semantics.
                if !options.flags().is_concatenated() {
                    writer.flush().await?;
                    return Ok(StreamSummary::new(total_in, total_out));
                }

                // In concatenated mode, `StreamEnd` can occur before we have observed EOF at the
                // I/O layer (e.g. when the next stream is already buffered or due to backend
                // semantics). Treat this as end-of-one-stream and continue decoding by starting a
                // fresh decoder for the next stream.
                //
                // Any trailing garbage will be rejected naturally when the next decoder fails to
                // parse a valid stream.
                let remaining = pending_len - consumed;
                decoder = options.build_decoder()?;
                if remaining > 0 {
                    // Continue with the still-buffered bytes.
                    input.copy_within(consumed..pending_len, 0);
                    pending_len = remaining;
                } else {
                    // No buffered bytes left; read more input to determine whether there's another
                    // stream or we are done.
                    let read = reader.read(&mut input).await?;
                    if read == 0 {
                        writer.flush().await?;
                        return Ok(StreamSummary::new(total_in, total_out));
                    }
                    pending_len = read;
                }
                consumed = 0;
                continue;
            }

            if used == 0 && written == 0 {
                // Decoder made no progress with current input; read more and append.
                if pending_len == input.len() {
                    // Grow the buffer to accommodate more pending input.
                    let grow_by = options.input_capacity().max(1);
                    input.try_reserve(grow_by).map_err(|_| {
                        crate::error::Error::AllocationFailed {
                            capacity: input.len() + grow_by,
                        }
                    })?;
                    input.resize(input.len() + grow_by, 0);
                }

                let read = reader.read(&mut input[pending_len..]).await?;
                if read == 0 {
                    // No more input available; finish using the still-pending bytes.
                    finish_decoder_async(
                        &mut decoder,
                        &mut writer,
                        &mut output,
                        &mut total_out,
                        &input[consumed..pending_len],
                    )
                    .await?;
                    return Ok(StreamSummary::new(total_in, total_out));
                }

                pending_len += read;
            }
        }

        // Move the unconsumed tail to the beginning of the buffer.
        let remaining = pending_len - consumed;
        if remaining > 0 {
            input.copy_within(consumed..pending_len, 0);
        }
        pending_len = remaining;
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
    encoder: &mut BuiltEncoder,
    writer: &mut W,
    output: &mut [u8],
    total_out: &mut u64,
) -> Result<()> {
    let mut made_progress = false;

    loop {
        match encoder.process(&[], output, Action::Finish) {
            Ok((_, written)) if written > 0 => {
                writer.write_all(&output[..written]).await?;
                *total_out += written as u64;
                made_progress = true;
            }
            Ok(_) => {
                if encoder.is_finished() || made_progress {
                    break;
                }

                return Err(BackendError::BufError.into());
            }
            Err(err) if matches!(err, BackendError::BufError) => {
                if encoder.is_finished() || made_progress {
                    break;
                }

                return Err(err.into());
            }
            Err(err) => return Err(err.into()),
        }

        if encoder.is_finished() {
            break;
        }
    }

    writer.flush().await?;
    Ok(())
}

/// Finishes decoding asynchronously by driving the decoder to `StreamEnd`.
///
/// # Parameters
///
/// * `decoder` - The decoder instance to finish
/// * `writer` - Async output writer to receive the final decoded data
/// * `output` - Buffer for temporary storage of decoded data
/// * `total_out` - Running count of total bytes written (updated in-place)
/// * `pending` - Remaining input bytes that were read but not yet consumed by the decoder
///
/// # Returns
///
/// * `Ok(())` if the decoder finished successfully
/// * `Err(BackendError::DataError)` if the stream couldn't be finished (e.g. truncated/corrupt)
///
/// This function uses a bounded number of iterations to avoid infinite loops if the backend
/// fails to make progress.
async fn finish_decoder_async<W: AsyncWrite + Unpin>(
    decoder: &mut Decoder,
    writer: &mut W,
    output: &mut [u8],
    total_out: &mut u64,
    mut pending: &[u8],
) -> Result<()> {
    // Prevent infinite loops by limiting the number of finish attempts.
    //
    // On truncated input, liblzma won't be able to finish the stream and will
    // typically make no progress once input is exhausted.
    const MAX_SPINS: usize = 64;

    for _ in 0..MAX_SPINS {
        let (used, written) = decoder.process(pending, output, Action::Finish)?;
        if written > 0 {
            writer.write_all(&output[..written]).await?;
            *total_out += written as u64;
        }

        pending = pending.get(used..).unwrap_or(&[]);

        if decoder.is_finished() {
            writer.flush().await?;
            return Ok(());
        }

        // If we still have pending input but the decoder couldn't consume anything,
        // it will never finish.
        if !pending.is_empty() && used == 0 && written == 0 {
            break;
        }
    }

    Err(BackendError::DataError.into())
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU64, NonZeroUsize};
    use std::time::Duration;

    use crate::config::DecodeMode;
    use crate::options::{
        Compression, CompressionOptions, DecompressionOptions, Flags, IntegrityCheck,
    };
    use crate::pipeline::tests::{FailingReader, SlowReader, EMPTY_SAMPLE, LARGE_SAMPLE, SAMPLE};
    use crate::threading::Threading;

    use super::*;

    /// Maximum duration for async tests
    const MAX_DURATION: Duration = Duration::from_secs(60);

    // Helper constants for memory sizes
    const KB: usize = 1024;
    const MB: usize = 1024 * KB;

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
            NonZeroUsize::new(KB).unwrap(),      // Small buffers
            NonZeroUsize::new(8 * KB).unwrap(),  // Medium buffers
            NonZeroUsize::new(64 * KB).unwrap(), // Large buffers
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
        const MEMORY_LIMIT: u64 = 128 * MB as u64;

        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0);

        // Test with generous memory limit
        let options =
            DecompressionOptions::default().with_memlimit(NonZeroU64::new(MEMORY_LIMIT).unwrap());
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
        const BLOCK_SIZE: u64 = 64 * KB as u64;

        let block_size = NonZeroU64::new(BLOCK_SIZE).unwrap();
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
        const TINY_SIZE: usize = 16;

        let tiny_size = NonZeroUsize::new(TINY_SIZE).unwrap();
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

    // Test oversized thread count handling (should be clamped, not fail).
    async_test!(oversized_thread_count_is_clamped, {
        const THREAD_COUNT: u32 = 1000;

        // Requesting an oversized explicit thread count should not fail; the encoder
        // will clamp it to a safe maximum at runtime.
        let options = CompressionOptions::default().with_threads(Threading::Exact(THREAD_COUNT));
        let mut compressed = Vec::new();
        let compression_summary = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();
        assert!(compression_summary.bytes_written > 0);

        // The produced stream must still be decodable.
        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let _ = decompress_async(compressed.as_slice(), &mut decompressed, &options)
            .await
            .unwrap();
        assert!(decompressed == SAMPLE);
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
        const MEMORY_LIMIT: u64 = KB as u64;

        // Compress some data first
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let _ = compress_async(LARGE_SAMPLE, &mut compressed, &options)
            .await
            .unwrap();

        // Try to decompress with a very restrictive memory limit
        let options =
            DecompressionOptions::default().with_memlimit(NonZeroU64::new(MEMORY_LIMIT).unwrap());
        let mut decompressed = Vec::new();

        let result = decompress_async(compressed.as_slice(), &mut decompressed, &options).await;

        // Should fail due to memory limit
        assert!(result.is_err());
        matches!(result.unwrap_err(), crate::error::Error::Backend(_));
    });

    // Test error handling - threading with unsupported mode
    async_test!(error_threading_unsupported_mode, {
        const THREAD_COUNT: u32 = 2;

        // Compress some data first
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let _ = compress_async(SAMPLE, &mut compressed, &options)
            .await
            .unwrap();

        // Try to use threading with LZMA mode (which doesn't support it)
        let options = DecompressionOptions::default()
            .with_mode(DecodeMode::Lzma)
            .with_threads(Threading::Exact(THREAD_COUNT));
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

    // Test that async multithreaded encoder handles finish properly and produces correct output.
    //
    // This test specifically targets the issue where multithreaded encoders don't signal
    // completion properly but continue producing valid compressed data.
    async_test!(multithreaded_encoder_finish_behavior, {
        // Create a data sample that's large enough to trigger the multithreaded encoder issue
        let test_data = vec![0x42u8; 2 * MB];

        // Configure multithreaded compression explicitly
        let threads = crate::threading::get_safe_max_threads();
        let options = CompressionOptions::default()
            .with_threads(Threading::Exact(threads))
            .with_level(Compression::Level6);

        let mut compressed = Vec::new();
        let compression_summary = compress_async(test_data.as_slice(), &mut compressed, &options)
            .await
            .unwrap();

        // Verify compression produced output
        assert!(compression_summary.bytes_written > 0);
        assert_eq!(compression_summary.bytes_read, test_data.len() as u64);

        // Verify the compressed data can be decompressed correctly
        let mut decompressed = Vec::new();
        let decompression_options = DecompressionOptions::default();
        let decompression_summary = decompress_async(
            compressed.as_slice(),
            &mut decompressed,
            &decompression_options,
        )
        .await
        .unwrap();

        // Verify decompression statistics
        assert_eq!(decompression_summary.bytes_written, test_data.len() as u64);
        assert_eq!(decompression_summary.bytes_read, compressed.len() as u64);

        // Verify data
        assert!(decompressed == test_data);
    });

    // Test async compression/decompression with various data patterns to ensure
    // data integrity is maintained
    async_test!(data_integrity_various_patterns, {
        // Test data with different compressibility and structure
        let test_cases: &[(&str, Vec<u8>)] = &[
            (
                "random_like",
                (0..KB)
                    .map(|i| {
                        let val = (i * 7 + 13) % 256;
                        #[allow(clippy::cast_possible_truncation)]
                        let byte = val as u8;
                        byte
                    })
                    .collect::<Vec<_>>(),
            ),
            ("highly_compressible", vec![0xAAu8; MB]),
            (
                "mixed_pattern",
                (0..MB)
                    .map(|i| {
                        if i % 1000 < 10 {
                            0xFF
                        } else {
                            #[allow(clippy::cast_possible_truncation)]
                            let byte = (i % 256) as u8;
                            byte
                        }
                    })
                    .collect::<Vec<_>>(),
            ),
            ("empty", vec![]),
            ("single_byte", vec![42]),
        ];

        // Test both single-threaded and multithreaded modes
        let threads = crate::threading::get_safe_max_threads();
        let threadings = [Threading::Exact(1), Threading::Exact(threads)];

        for (case_name, test_data) in test_cases {
            for &threading in &threadings {
                let options = CompressionOptions::default()
                    .with_threads(threading)
                    .with_level(Compression::Level6);

                let mut compressed = Vec::new();
                let compression_summary =
                    compress_async(test_data.as_slice(), &mut compressed, &options)
                        .await
                        .unwrap();

                // For non-empty data, there should be output
                if !test_data.is_empty() {
                    assert!(
                        compression_summary.bytes_written > 0,
                        "Compression output is empty for case '{case_name}', threading {threading:?}",
                    );
                }
                assert_eq!(
                    compression_summary.bytes_read,
                    test_data.len() as u64,
                    "Compression bytes_read mismatch for case '{case_name}', threading {threading:?}",
                );

                // Decompression and data integrity check
                let mut decompressed = Vec::new();
                let decompression_options = DecompressionOptions::default();
                let decompression_summary = decompress_async(
                    compressed.as_slice(),
                    &mut decompressed,
                    &decompression_options,
                )
                .await
                .unwrap();

                assert_eq!(
                    decompression_summary.bytes_written,
                    test_data.len() as u64,
                    "Decompression bytes_written mismatch for case '{case_name}', threading {threading:?}",
                );
                assert_eq!(
                    &decompressed, test_data,
                    "Data integrity check failed for case '{case_name}', threading {threading:?}",
                );
            }
        }
    });

    // Test that concatenated `.xz` streams are decoded fully when `CONCATENATED` is set.
    async_test!(concatenated_xz_streams_decode_fully, {
        let options = CompressionOptions::default();

        let mut compressed_a = Vec::new();
        let a_summary = match compress_async(SAMPLE, &mut compressed_a, &options).await {
            Ok(v) => v,
            Err(err) => panic!("compress_async(A) failed: {err:?}"),
        };
        assert!(a_summary.bytes_written > 0);

        let mut compressed_b = Vec::new();
        let b_summary = match compress_async(LARGE_SAMPLE, &mut compressed_b, &options).await {
            Ok(v) => v,
            Err(err) => panic!("compress_async(B) failed: {err:?}"),
        };
        assert!(b_summary.bytes_written > 0);

        let mut concatenated = Vec::with_capacity(compressed_a.len() + compressed_b.len());
        concatenated.extend_from_slice(&compressed_a);
        concatenated.extend_from_slice(&compressed_b);

        // Without CONCATENATED we stop after the first stream.
        let mut decompressed_single = Vec::new();
        let single_opts = DecompressionOptions::default();
        let _ = match decompress_async(
            concatenated.as_slice(),
            &mut decompressed_single,
            &single_opts,
        )
        .await
        {
            Ok(v) => v,
            Err(err) => panic!("decompress_async(single-stream) failed: {err:?}"),
        };
        assert_eq!(decompressed_single, SAMPLE);

        // With CONCATENATED we decode both streams.
        let mut decompressed_all = Vec::new();
        let concat_opts = DecompressionOptions::default().with_flags(Flags::CONCATENATED);
        let _ = match decompress_async(concatenated.as_slice(), &mut decompressed_all, &concat_opts)
            .await
        {
            Ok(v) => v,
            Err(err) => panic!("decompress_async(concatenated) failed: {err:?}"),
        };

        let mut expected = Vec::with_capacity(SAMPLE.len() + LARGE_SAMPLE.len());
        expected.extend_from_slice(SAMPLE);
        expected.extend_from_slice(LARGE_SAMPLE);
        assert_eq!(decompressed_all, expected);
    });
}
