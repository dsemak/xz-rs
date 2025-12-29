//! Synchronous XZ compression and decompression pipeline.

use std::io::{Read, Write};

use lzma_safe::{Action, Decoder};

use crate::buffer::Buffer;
use crate::config::StreamSummary;
use crate::error::{BackendError, Result};
use crate::options::{BuiltEncoder, CompressionOptions, DecompressionOptions};

/// Compresses data from a reader into a writer using the provided options.
///
/// # Parameters
///
/// * `reader` - Input source implementing [`Read`] trait
/// * `writer` - Output destination implementing [`Write`] trait
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
/// - I/O operations on reader or writer fail
/// - Invalid compression parameters are specified
/// - Threading limits are exceeded
pub fn compress<R, W>(
    mut reader: R,
    mut writer: W,
    options: &CompressionOptions,
) -> Result<StreamSummary>
where
    R: Read,
    W: Write,
{
    let mut encoder = options.build_encoder()?;
    let mut input = Buffer::new(options.input_capacity())?;
    let mut output = Buffer::new(options.output_capacity())?;
    let mut total_in = 0u64;
    let mut total_out = 0u64;

    loop {
        let read = reader.read(&mut input)?;
        if read == 0 {
            finish_encoder_sync(&mut encoder, &mut writer, &mut output, &mut total_out)?;
            return Ok(StreamSummary::new(total_in, total_out));
        }

        let mut consumed = 0usize;
        while consumed < read {
            let (used, written) =
                encoder.process(&input[consumed..read], &mut output, Action::Run)?;
            if written > 0 {
                writer.write_all(&output[..written])?;
                total_out += written as u64;
            }
            consumed += used;
            total_in += used as u64;

            if encoder.is_finished() {
                writer.flush()?;
                return Ok(StreamSummary::new(total_in, total_out));
            }

            if used == 0 && written == 0 {
                break;
            }
        }
    }
}

/// Decompresses data from a reader into a writer using the provided options.
///
/// # Parameters
///
/// * `reader` - Input source implementing [`Read`] trait
/// * `writer` - Output destination implementing [`Write`] trait
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
/// - I/O operations on reader or writer fail
/// - The compressed data is corrupted or invalid
/// - Memory limits are exceeded during decompression
/// - Threading is requested for unsupported decode modes
pub fn decompress<R, W>(
    mut reader: R,
    mut writer: W,
    options: &DecompressionOptions,
) -> Result<StreamSummary>
where
    R: Read,
    W: Write,
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
            let read = reader.read(&mut input)?;
            if read == 0 {
                // If no data was ever processed, this is an invalid/empty input
                if total_in == 0 {
                    return Err(BackendError::DataError.into());
                }
                finish_decoder_sync(&mut decoder, &mut writer, &mut output, &mut total_out, &[])?;
                return Ok(StreamSummary::new(total_in, total_out));
            }
            pending_len = read;
        }

        let mut consumed = 0usize;
        while consumed < pending_len {
            let (used, written) =
                decoder.process(&input[consumed..pending_len], &mut output, Action::Run)?;
            if written > 0 {
                writer.write_all(&output[..written])?;
                total_out += written as u64;
            }
            consumed += used;
            total_in += used as u64;

            if decoder.is_finished() {
                // In non-concatenated mode, "finished" means we intentionally stop after the first
                // stream and ignore any remaining input. This matches the xz(1) --single-stream
                // semantics.
                if !options.flags().is_concatenated() {
                    writer.flush()?;
                    return Ok(StreamSummary::new(total_in, total_out));
                }

                // In concatenated mode, finishing before EOF indicates trailing garbage or an
                // unexpected state. Be strict and require EOF.
                let remaining = pending_len - consumed;
                if remaining > 0 {
                    return Err(BackendError::DataError.into());
                }
                let read = reader.read(&mut input)?;
                if read == 0 {
                    writer.flush()?;
                    return Ok(StreamSummary::new(total_in, total_out));
                }
                return Err(BackendError::DataError.into());
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

                let read = reader.read(&mut input[pending_len..])?;
                if read == 0 {
                    // No more input available; finish using the still-pending bytes.
                    finish_decoder_sync(
                        &mut decoder,
                        &mut writer,
                        &mut output,
                        &mut total_out,
                        &input[consumed..pending_len],
                    )?;
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

/// Finishes the encoding process by flushing any remaining data from the encoder.
///
/// # Parameters
///
/// * `encoder` - The encoder instance to finish
/// * `writer` - Output writer to receive the final compressed data
/// * `output` - Buffer for temporary storage of compressed data
/// * `total_out` - Running count of total bytes written (updated in-place)
///
/// # Returns
///
/// * `Ok(())` if the encoder finished successfully
/// * `Err(BackendError::BufError)` if the encoder gets stuck in an infinite loop
fn finish_encoder_sync<W: Write>(
    encoder: &mut BuiltEncoder,
    writer: &mut W,
    output: &mut [u8],
    total_out: &mut u64,
) -> Result<()> {
    let mut made_progress = false;

    loop {
        match encoder.process(&[], output, Action::Finish) {
            Ok((_, written)) if written > 0 => {
                writer.write_all(&output[..written])?;
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

    writer.flush()?;
    Ok(())
}

/// Finishes decoding by driving the decoder to `StreamEnd`.
///
/// # Parameters
///
/// * `decoder` - The decoder instance to finish
/// * `writer` - Output writer to receive the final decoded data
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
fn finish_decoder_sync<W: Write>(
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
            writer.write_all(&output[..written])?;
            *total_out += written as u64;
        }

        pending = pending.get(used..).unwrap_or(&[]);

        if decoder.is_finished() {
            writer.flush()?;
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
    use std::io::Cursor;
    use std::num::{NonZeroU64, NonZeroUsize};
    use std::time::Duration;

    use crate::config::DecodeMode;
    use crate::options::{Compression, CompressionOptions, DecompressionOptions, IntegrityCheck};
    use crate::pipeline::tests::{
        FailingReader, FailingWriter, SlowReader, EMPTY_SAMPLE, LARGE_SAMPLE, SAMPLE,
    };
    use crate::threading::Threading;

    use super::*;

    /// Test basic round-trip compression and decompression functionality.
    #[test]
    fn sync_round_trip_works() {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0);
        assert_eq!(
            usize::try_from(compression_summary.bytes_read).unwrap(),
            SAMPLE.len()
        );

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let decompression_summary =
            decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
        assert_eq!(
            usize::try_from(decompression_summary.bytes_written).unwrap(),
            SAMPLE.len()
        );
        assert!(decompressed == SAMPLE);
    }

    /// Test compression and decompression of empty input.
    #[test]
    fn sync_empty_input() {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress(EMPTY_SAMPLE, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0); // XZ header is always present

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let decompression_summary =
            decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
        assert_eq!(decompression_summary.bytes_written, 0);
        assert!(decompressed == EMPTY_SAMPLE);
    }

    /// Test compression and decompression of large input data.
    #[test]
    fn sync_large_input() {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress(LARGE_SAMPLE, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0);
        assert!(compression_summary.bytes_read == LARGE_SAMPLE.len() as u64);

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let decompression_summary =
            decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
        assert_eq!(
            usize::try_from(decompression_summary.bytes_written).unwrap(),
            LARGE_SAMPLE.len()
        );
        assert!(decompressed == LARGE_SAMPLE);
    }

    /// Test compression with different compression levels.
    #[test]
    fn sync_compression_levels() {
        let levels = [
            Compression::Level0,
            Compression::Level1,
            Compression::Level6,
            Compression::Level9,
        ];

        for level in levels {
            let options = CompressionOptions::default().with_level(level);
            let mut compressed = Vec::new();
            let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
            assert!(compression_summary.bytes_written > 0);

            let mut decompressed = Vec::new();
            let options = DecompressionOptions::default();
            let _ = decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
            assert!(decompressed == SAMPLE);
        }
    }

    /// Test different integrity checks
    #[test]
    fn sync_integrity_checks() {
        let checks = [
            IntegrityCheck::None,
            IntegrityCheck::Crc32,
            IntegrityCheck::Crc64,
        ];

        for check in checks {
            let options = CompressionOptions::default().with_check(check);
            let mut compressed = Vec::new();
            let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
            assert!(compression_summary.bytes_written > 0);

            let mut decompressed = Vec::new();
            let options = DecompressionOptions::default();
            let _ = decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
            assert!(decompressed == SAMPLE);
        }
    }

    /// Test different buffer sizes
    #[test]
    fn sync_buffer_sizes() {
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
            let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
            assert!(compression_summary.bytes_written > 0);

            let decompression_options = DecompressionOptions::default()
                .with_input_buffer_size(size)
                .with_output_buffer_size(size);
            let mut decompressed = Vec::new();
            let _ = decompress(
                compressed.as_slice(),
                &mut decompressed,
                &decompression_options,
            )
            .unwrap();
            assert!(decompressed == SAMPLE);
        }
    }

    /// Test threading configurations
    #[test]
    fn sync_threading_options() {
        // Only test single-threaded mode to avoid threading issues
        let thread_configs = [Threading::Auto, Threading::Exact(1)];

        for threads in thread_configs {
            let options = CompressionOptions::default().with_threads(threads);
            let mut compressed = Vec::new();
            let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
            assert!(compression_summary.bytes_written > 0);

            // For decompression, only use single-threaded mode to avoid threading issues
            let decompression_options =
                DecompressionOptions::default().with_threads(Threading::Exact(1));
            let mut decompressed = Vec::new();
            let _ = decompress(
                compressed.as_slice(),
                &mut decompressed,
                &decompression_options,
            )
            .unwrap();
            assert!(decompressed == SAMPLE);
        }
    }

    /// Test memory limits for decompression
    #[test]
    fn sync_memory_limits() {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0);

        // Test with generous memory limit
        let options = DecompressionOptions::default()
            .with_memlimit(NonZeroU64::new(128 * 1024 * 1024).unwrap());
        let mut decompressed = Vec::new();
        let _ = decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
        assert!(decompressed == SAMPLE);
    }

    /// Test decode modes
    #[test]
    fn sync_decode_modes() {
        let modes = [DecodeMode::Auto, DecodeMode::Xz];

        for mode in modes {
            let mut compressed = Vec::new();
            let options = CompressionOptions::default();
            let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
            assert!(compression_summary.bytes_written > 0);

            let options = DecompressionOptions::default().with_mode(mode);
            let mut decompressed = Vec::new();
            let _ = decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
            assert!(decompressed == SAMPLE);
        }
    }

    /// Test with timeout configuration
    #[test]
    fn sync_with_timeout() {
        let timeout = Duration::from_millis(1000);
        let options = CompressionOptions::default().with_timeout(Some(timeout));
        let mut compressed = Vec::new();
        let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0);

        let decompression_options = DecompressionOptions::default().with_timeout(Some(timeout));
        let mut decompressed = Vec::new();
        let _ = decompress(
            compressed.as_slice(),
            &mut decompressed,
            &decompression_options,
        )
        .unwrap();
        assert!(decompressed == SAMPLE);
    }

    /// Test with block size configuration
    #[test]
    fn sync_with_block_size() {
        let block_size = NonZeroU64::new(64 * 1024).unwrap();
        let options = CompressionOptions::default().with_block_size(Some(block_size));
        let mut compressed = Vec::new();
        let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0);

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let _ = decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
        assert!(decompressed == SAMPLE);
    }

    /// Test streaming with small chunks
    #[test]
    fn sync_streaming_small_chunks() {
        // Read 4 bytes at a time
        let reader = SlowReader::new(SAMPLE, 4);
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress(reader, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0);

        let reader = SlowReader::new(&compressed, 8); // Read 8 bytes at a time
        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let _ = decompress(reader, &mut decompressed, &options).unwrap();
        assert!(decompressed == SAMPLE);
    }

    /// Test summary statistics accuracy
    #[test]
    fn sync_summary_statistics() {
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();

        assert_eq!(compression_summary.bytes_read, SAMPLE.len() as u64);
        assert_eq!(compression_summary.bytes_written, compressed.len() as u64);
        assert!(compression_summary.bytes_written > 0);

        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let decompression_summary =
            decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();

        assert_eq!(decompression_summary.bytes_read, compressed.len() as u64);
        assert_eq!(decompression_summary.bytes_written, SAMPLE.len() as u64);
    }

    /// Test with very small buffers to stress internal buffering
    #[test]
    fn sync_tiny_buffers() {
        let tiny_size = NonZeroUsize::new(16).unwrap();
        let options = CompressionOptions::default()
            .with_input_buffer_size(tiny_size)
            .with_output_buffer_size(tiny_size);

        let mut compressed = Vec::new();
        let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0);

        let decompression_options = DecompressionOptions::default()
            .with_input_buffer_size(tiny_size)
            .with_output_buffer_size(tiny_size);
        let mut decompressed = Vec::new();
        let _ = decompress(
            compressed.as_slice(),
            &mut decompressed,
            &decompression_options,
        )
        .unwrap();
        assert!(decompressed == SAMPLE);
    }

    /// Test multiple consecutive operations
    #[test]
    fn sync_multiple_operations() {
        for _ in 0..5 {
            let mut compressed = Vec::new();
            let options = CompressionOptions::default();
            let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
            assert!(compression_summary.bytes_written > 0);

            let mut decompressed = Vec::new();
            let options = DecompressionOptions::default();
            let _ = decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
            assert!(decompressed == SAMPLE);
        }
    }

    /// Test oversized thread count handling (should be clamped, not fail).
    #[test]
    fn sync_oversized_thread_count_is_clamped() {
        // Requesting an oversized explicit thread count should not fail; the encoder
        // will clamp it to a safe maximum at runtime.
        let options = CompressionOptions::default().with_threads(Threading::Exact(1000));
        let mut compressed = Vec::new();
        let compression_summary = compress(SAMPLE, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0);

        // The produced stream must still be decodable.
        let mut decompressed = Vec::new();
        let options = DecompressionOptions::default();
        let _ = decompress(compressed.as_slice(), &mut decompressed, &options).unwrap();
        assert!(decompressed == SAMPLE);
    }

    /// Test error handling - corrupted data
    #[test]
    fn sync_error_corrupted_data() {
        // Create some invalid compressed data
        let corrupted_data = b"This is not valid XZ data";
        let mut decompressed = Vec::new();

        let options = DecompressionOptions::default();
        let result = decompress(corrupted_data.as_slice(), &mut decompressed, &options);

        // Should fail with a backend error
        assert!(result.is_err());
        matches!(result.unwrap_err(), crate::error::Error::Backend(_));
    }

    /// Test error handling - memory limit exceeded
    #[test]
    fn sync_error_memory_limit() {
        // Compress some data first
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let _ = compress(LARGE_SAMPLE, &mut compressed, &options).unwrap();

        // Try to decompress with a very restrictive memory limit
        let options = DecompressionOptions::default().with_memlimit(NonZeroU64::new(1024).unwrap());
        let mut decompressed = Vec::new();

        let result = decompress(compressed.as_slice(), &mut decompressed, &options);

        // Should fail due to memory limit
        assert!(result.is_err());
        matches!(result.unwrap_err(), crate::error::Error::Backend(_));
    }

    /// Test error handling - threading with unsupported mode
    #[test]
    fn sync_error_threading_unsupported_mode() {
        // Compress some data first
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let _ = compress(SAMPLE, &mut compressed, &options).unwrap();

        // Try to use threading with LZMA mode (which doesn't support it)
        let options = DecompressionOptions::default()
            .with_mode(DecodeMode::Lzma)
            .with_threads(Threading::Exact(2));
        let mut decompressed = Vec::new();

        let result = decompress(compressed.as_slice(), &mut decompressed, &options);

        // Should fail with ThreadingUnsupported error
        assert!(result.is_err());
        if let Err(crate::error::Error::ThreadingUnsupported { requested, mode }) = result {
            assert_eq!(requested, 2);
            assert_eq!(mode, DecodeMode::Lzma);
        } else {
            panic!("Expected ThreadingUnsupported error, got: {result:?}");
        }
    }

    /// Test error handling - I/O errors during reading
    #[test]
    fn sync_error_io_failure() {
        // Fail after 10 bytes
        let failing_reader = FailingReader::new(10);
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();

        let result = compress(failing_reader, &mut compressed, &options);

        // Should fail with I/O error
        assert!(result.is_err());
        matches!(result.unwrap_err(), crate::error::Error::Io(_));
    }

    /// Test error handling - I/O errors during writing
    #[test]
    fn sync_error_write_failure() {
        // Fail after 5 bytes
        let failing_writer = FailingWriter::new(5);
        let options = CompressionOptions::default();
        let result = compress(SAMPLE, failing_writer, &options);

        // Should fail with I/O error
        assert!(result.is_err());
        matches!(result.unwrap_err(), crate::error::Error::Io(_));
    }

    /// Test error handling - very small buffer sizes
    #[test]
    fn sync_error_small_buffer_sizes() {
        let small_size = NonZeroUsize::new(64).unwrap();
        let options = CompressionOptions::default()
            .with_input_buffer_size(small_size)
            .with_output_buffer_size(small_size);

        let mut compressed = Vec::new();
        let result = compress(SAMPLE, &mut compressed, &options);

        // Should work fine with small buffer sizes
        assert!(result.is_ok());
    }

    /// Test with cursor-based I/O
    #[test]
    fn sync_cursor_io() {
        let mut input_cursor = Cursor::new(SAMPLE);
        let mut compressed = Vec::new();
        let options = CompressionOptions::default();
        let compression_summary = compress(&mut input_cursor, &mut compressed, &options).unwrap();
        assert!(compression_summary.bytes_written > 0);

        let mut compressed_cursor = Cursor::new(compressed);
        let options = DecompressionOptions::default();
        let mut decompressed = Vec::new();
        let _ = decompress(&mut compressed_cursor, &mut decompressed, &options).unwrap();
        assert!(decompressed == SAMPLE);
    }
}
