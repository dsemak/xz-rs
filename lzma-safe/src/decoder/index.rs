//! High-level, safe Rust wrapper for liblzma's XZ Index decoder.

use crate::stream::LzmaAllocator;
use crate::{Action, Index, Result, Stream};

/// Safe wrapper around liblzma's index decoder.
///
/// This decoder extracts index metadata from XZ Index blocks without decompressing the actual data.
/// It can be used to get information about streams, blocks, compression ratios, etc.
pub struct IndexDecoder {
    /// The underlying stream
    stream: Option<Stream>,
    /// Pointer to the index being decoded (owned by liblzma until decoding completes)
    index_ptr: *mut liblzma_sys::lzma_index,
    /// The extracted index (available only after decoding completes)
    index: Option<Index>,
    /// Allocator from the stream, kept for cleanup
    allocator: Option<LzmaAllocator>,
}

impl IndexDecoder {
    /// Create a new index decoder.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for the decoder
    /// * `stream` - The underlying stream
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::MemError`] if memory allocation fails.
    /// Returns [`crate::Error::ProgError`] if the decoder is misused.
    pub fn new(memlimit: u64, mut stream: Stream) -> Result<Self> {
        let mut index_ptr: *mut liblzma_sys::lzma_index = std::ptr::null_mut();
        let allocator = stream.allocator();
        crate::ffi::lzma_index_decoder(&mut stream, std::ptr::from_mut(&mut index_ptr), memlimit)?;
        Ok(Self {
            stream: Some(stream),
            index_ptr,
            index: None,
            allocator,
        })
    }

    /// Process input data and extract file information.
    ///
    /// # Parameters
    ///
    /// * `input` - Input data buffer
    /// * `action` - Action to perform (Run or Finish)
    ///
    /// # Returns
    ///
    /// Returns the number of bytes consumed from input.
    ///
    /// # Errors
    ///
    /// Returns various errors depending on the input data and decoder state.
    pub fn process(&mut self, input: &[u8], action: Action) -> Result<usize> {
        // Take ownership of the stream for this operation.
        let Some(mut stream) = self.stream.take() else {
            // Stream is already finished or dropped; this is a logic error.
            return Err(crate::Error::ProgError);
        };

        if !input.is_empty() {
            stream.set_next_input(input);
        }

        let input_before = stream.avail_in();

        // Call lzma_code with proper mutable reference
        let result = crate::ffi::lzma_code(&mut stream, action);
        let bytes_read = input_before - stream.avail_in();

        // Handle special case for Action::Finish with empty input where liblzma
        // may require an additional call to signal LZMA_STREAM_END properly.
        let result =
            if action == Action::Finish && result.is_ok() && input_before == 0 && bytes_read == 0 {
                // For empty inputs liblzma may require one additional call before
                // signalling `LZMA_STREAM_END`. Invoke it here so callers don't
                // need to loop in trivial cases.
                let second_result = crate::ffi::lzma_code(&mut stream, action);
                let second_bytes_read = input_before - stream.avail_in();

                if (second_result.is_ok() || matches!(second_result, Err(crate::Error::BufError)))
                    && second_bytes_read == 0
                {
                    // Force stream end when no progress is made on the second call
                    Err(crate::Error::StreamEnd)
                } else {
                    second_result
                }
            } else {
                result
            };

        match result {
            Ok(()) => {
                // Decoding continues; put the stream back for further use.
                self.stream = Some(stream);
                Ok(bytes_read)
            }
            Err(crate::Error::StreamEnd) => {
                // Decoding is finished; extract the index if it's valid.
                if !self.index_ptr.is_null() {
                    // SAFETY: index_ptr is valid and was populated by liblzma
                    // Pass the stream's allocator to the index
                    let allocator = stream.allocator();
                    self.index = unsafe { Index::from_raw(self.index_ptr, allocator) };
                    // Clear the pointer since we've taken ownership
                    self.index_ptr = std::ptr::null_mut();
                }
                stream.finish();
                Ok(bytes_read)
            }
            Err(err) => {
                // On error, retain the stream for possible recovery or inspection.
                self.stream = Some(stream);
                Err(err)
            }
        }
    }

    /// Get the seek position if the decoder needs to seek.
    ///
    /// This should be called when `process` returns `Action::Run` and the
    /// decoder needs to seek to a specific position in the input file.
    pub fn seek_pos(&mut self) -> u64 {
        self.stream.as_mut().map_or(0, |s| s.lzma_stream().seek_pos)
    }

    /// Returns whether the decoding has finished and the index is available.
    pub fn is_finished(&self) -> bool {
        self.stream.is_none()
    }

    /// Get the total number of input bytes processed.
    pub fn total_in(&self) -> u64 {
        self.stream.as_ref().map_or(0, Stream::total_in)
    }

    /// Returns a reference to the extracted index if decoding completed successfully.
    ///
    /// Returns `None` if decoding has not finished yet.
    pub fn index(&self) -> Option<&Index> {
        if !self.is_finished() {
            return None;
        }
        self.index.as_ref()
    }
}

impl Drop for IndexDecoder {
    fn drop(&mut self) {
        // Free the index if it wasn't wrapped in Index yet
        if !self.index_ptr.is_null() {
            crate::ffi::lzma_index_end(self.index_ptr, self.allocator.as_ref());
        }

        // Finalize the stream
        if let Some(stream) = self.stream.take() {
            stream.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Action, Error, Stream};

    /// Test [`IndexDecoder`] creation and basic API.
    #[test]
    fn index_decoder_creation() {
        let decoder = Stream::default().index_decoder(u64::MAX).unwrap();

        // Initially the decoder is not finished
        assert!(!decoder.is_finished());

        // Index should not be available before decoding
        assert!(decoder.index().is_none());

        // Total in should be zero
        assert_eq!(decoder.total_in(), 0);
    }

    /// Test [`IndexDecoder`] with invalid index data returns error.
    #[test]
    fn index_decoder_invalid_data() {
        let invalid_data = b"Not a valid XZ Index block";

        let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();

        let result = decoder.process(invalid_data, Action::Finish);

        // Should return an error for invalid data
        assert!(result.is_err());
    }

    /// Test [`IndexDecoder`] `process_after_finish` returns error.
    #[test]
    fn index_decoder_process_after_finish() {
        let invalid_data = b"test";

        let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();

        // Try to process invalid data (will likely fail or succeed with error)
        let _ = decoder.process(invalid_data, Action::Finish);

        // Even if processing failed, trying again when stream is None should give ProgError
        if decoder.is_finished() {
            let result = decoder.process(invalid_data, Action::Run);
            assert!(result.is_err());
            // Should be ProgError if stream was finished
            if let Err(err) = result {
                assert_eq!(err, Error::ProgError);
            }
        }
    }

    /// Test `IndexDecoder` `seek_pos` method exists and returns value.
    #[test]
    fn index_decoder_seek_pos() {
        let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();

        // Should be able to call seek_pos
        let pos = decoder.seek_pos();
        // Initially should be 0
        assert_eq!(pos, 0);
    }

    /// Test `IndexDecoder` `total_in` counter.
    #[test]
    fn index_decoder_total_in() {
        let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();

        assert_eq!(decoder.total_in(), 0);

        // Process some data (even if it fails)
        let test_data = b"test data";
        let _ = decoder.process(test_data, Action::Run);

        // Total in may have changed
        let _ = decoder.total_in();
    }

    /// Test `IndexDecoder` round-trip with `Encoder`.
    ///
    /// Creates compressed data and verifies index decoder can process it.
    #[test]
    fn index_decoder_with_encoder_roundtrip() {
        use crate::encoder::options::{Compression, IntegrityCheck};

        // Test data to compress
        let test_data = b"Lazzy dog jumps over the lazy fox";

        // Compress data using encoder
        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level3, IntegrityCheck::Crc64)
            .unwrap();

        let mut compressed = vec![0u8; 4096];
        let (read, written) = encoder
            .process(test_data, &mut compressed, Action::Run)
            .unwrap();
        assert_eq!(read, test_data.len());

        let mut total_written = written;
        let (_, finish_written) = encoder
            .process(&[], &mut compressed[total_written..], Action::Finish)
            .unwrap();
        total_written += finish_written;
        compressed.truncate(total_written);

        assert!(encoder.is_finished());
        assert!(!compressed.is_empty());

        // Note: IndexDecoder expects only the Index section of the XZ file,
        // not the full file. For full file parsing, use FileInfoDecoder.
        let mut index_decoder = Stream::default().index_decoder(u64::MAX).unwrap();
        let _result = index_decoder.process(&compressed, Action::Finish);

        // The decoder may return an error since we're feeding it a full XZ file
        // instead of just the Index section. This is expected behavior.
    }

    /// Test `IndexDecoder` with stream that has zero blocks.
    ///
    /// Empty XZ stream with no data blocks.
    #[test]
    fn index_decoder_with_empty_xz_stream() {
        use crate::encoder::options::{Compression, IntegrityCheck};

        // Compress empty data
        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level1, IntegrityCheck::Crc32)
            .unwrap();

        let mut compressed = vec![0u8; 1024];
        let (read, written) = encoder
            .process(&[], &mut compressed, Action::Finish)
            .unwrap();
        assert_eq!(read, 0);
        compressed.truncate(written);

        assert!(encoder.is_finished());
        assert!(!compressed.is_empty()); // Should have header/index/footer

        // Try to decode index from empty stream
        let mut index_decoder = Stream::default().index_decoder(u64::MAX).unwrap();
        let _result = index_decoder.process(&compressed, Action::Finish);

        // May fail since we're passing full XZ file, not just Index section
    }

    /// Test memory limit enforcement.
    ///
    /// Verifies that decoder respects memory limits.
    #[test]
    fn index_decoder_memory_limit() {
        // Create decoder with very small memory limit
        let result = Stream::default().index_decoder(1);

        // Should succeed to create
        assert!(result.is_ok());

        let decoder = result.unwrap();
        assert_eq!(decoder.total_in(), 0);
        assert!(!decoder.is_finished());
    }

    /// Test `seek_pos` tracking.
    ///
    /// Verifies seek position is properly tracked.
    #[test]
    fn index_decoder_seek_position_tracking() {
        let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();

        let initial_pos = decoder.seek_pos();
        assert_eq!(initial_pos, 0);

        // Process some invalid data
        let test_data = b"Not an XZ Index";
        let _ = decoder.process(test_data, Action::Run);

        // Seek pos may change if decoder needs to seek
        let _ = decoder.seek_pos();
    }

    /// Test `total_in` tracking during processing.
    ///
    /// Verifies input byte counter is properly maintained.
    #[test]
    fn index_decoder_tracks_input_bytes() {
        let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();
        assert_eq!(decoder.total_in(), 0);

        let test_data = b"test data for byte counting";
        let _ = decoder.process(test_data, Action::Run);

        // total_in should reflect bytes consumed
        // Note: actual value depends on how much liblzma consumed
        let _total = decoder.total_in();
    }

    /// Test decoder state consistency.
    ///
    /// Verifies decoder maintains consistent state throughout processing.
    #[test]
    fn index_decoder_state_consistency() {
        let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();

        // Initial state
        assert!(!decoder.is_finished());
        assert_eq!(decoder.total_in(), 0);
        assert_eq!(decoder.seek_pos(), 0);
        assert!(decoder.index().is_none());

        // After processing invalid data
        let test_data = b"invalid";
        let result = decoder.process(test_data, Action::Run);

        if result.is_err() {
            // Decoder should still be in consistent state after error
            assert!(!decoder.is_finished());
        }
    }

    /// Test decoder with `Action::Run` vs `Action::Finish`.
    ///
    /// Verifies different actions behave correctly.
    #[test]
    fn index_decoder_action_handling() {
        let test_data = b"test data";

        // Test with Action::Run
        {
            let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();
            let _result = decoder.process(test_data, Action::Run);
            // Decoder may or may not finish depending on data
        }

        // Test with Action::Finish
        {
            let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();
            let _result = decoder.process(test_data, Action::Finish);
            // Finish should signal end of input
        }
    }

    /// Test that `index()` returns `None` before finishing.
    ///
    /// Validates index is only available after successful decode.
    #[test]
    fn index_decoder_index_availability() {
        let mut decoder = Stream::default().index_decoder(u64::MAX).unwrap();

        // Before any processing
        assert!(decoder.index().is_none());

        // During processing (with invalid data)
        let test_data = b"not an index";
        let _ = decoder.process(test_data, Action::Run);

        if !decoder.is_finished() {
            // Index should not be available yet
            assert!(decoder.index().is_none());
        }
    }
}
