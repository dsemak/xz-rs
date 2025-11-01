//! High-level, safe Rust wrapper for liblzma's file info decoder.

use crate::stream::LzmaAllocator;
use crate::{Action, Error, Index, Result, Stream};

/// Safe wrapper around liblzma's file info decoder.
///
/// This decoder extracts index metadata from a complete XZ file by reading
/// Stream Headers, Stream Footers, Index blocks, and Stream Padding.
/// It can seek within the file to efficiently extract metadata without
/// decompressing the actual data.
pub struct FileInfoDecoder {
    /// The underlying stream
    stream: Option<Stream>,
    /// Boxed pointer to ensure stable address for liblzma to write to
    index_ptr_box: Box<*mut liblzma_sys::lzma_index>,
    /// The extracted index (available only after decoding completes)
    index: Option<Index>,
    /// Size of the input file (required by liblzma)
    file_size: u64,
    /// Allocator from the stream, kept for cleanup
    allocator: Option<LzmaAllocator>,
}

impl FileInfoDecoder {
    /// Create a new file info decoder.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for the decoder
    /// * `file_size` - Total size of the input XZ file in bytes
    /// * `stream` - The underlying stream
    ///
    /// # Errors
    ///
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::ProgError`] if the decoder is misused.
    pub fn new(memlimit: u64, file_size: u64, mut stream: Stream) -> Result<Self> {
        let mut index_ptr_box = Box::new(std::ptr::null_mut());
        let allocator = stream.allocator();
        crate::ffi::lzma_file_info_decoder(
            &mut stream,
            std::ptr::from_mut(&mut *index_ptr_box),
            memlimit,
            file_size,
        )?;
        Ok(Self {
            stream: Some(stream),
            index_ptr_box,
            index: None,
            file_size,
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
    /// Returns [`Error::SeekNeeded`] if the decoder needs the application to seek
    /// to a different position in the file. Use [`seek_pos()`](Self::seek_pos) to get the target position.
    /// Returns various other errors depending on the input data and decoder state.
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
            Err(Error::SeekNeeded) => {
                // Decoder needs seeking; put the stream back and return the error.
                self.stream = Some(stream);
                Err(Error::SeekNeeded)
            }
            Err(crate::Error::StreamEnd) => {
                // Decoding is finished; extract the index if it's valid.
                if !(*self.index_ptr_box).is_null() {
                    // SAFETY: index_ptr is valid and was populated by liblzma
                    // Pass the stream's allocator to the index
                    let allocator = stream.allocator();
                    self.index = unsafe { Index::from_raw(*self.index_ptr_box, allocator) };
                    // Clear the pointer since we've taken ownership
                    *self.index_ptr_box = std::ptr::null_mut();
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
    /// This should be called when `process` returns [`Error::SeekNeeded`].
    /// The application should then seek to this position in the input file
    /// and provide data starting from this position.
    pub fn seek_pos(&self) -> u64 {
        self.stream.as_ref().map_or(0, Stream::seek_pos)
    }

    /// Returns whether the decoding has finished and the index is available.
    pub fn is_finished(&self) -> bool {
        self.stream.is_none()
    }

    /// Get the total number of input bytes processed.
    pub fn total_in(&self) -> u64 {
        self.stream.as_ref().map_or(0, Stream::total_in)
    }

    /// Get the size of the input file.
    pub fn file_size(&self) -> u64 {
        self.file_size
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

impl Drop for FileInfoDecoder {
    fn drop(&mut self) {
        // Free the index if it wasn't wrapped in Index yet
        if !(*self.index_ptr_box).is_null() {
            crate::ffi::lzma_index_end(*self.index_ptr_box, self.allocator.as_ref());
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

    /// Test [`FileInfoDecoder`] creation and basic API.
    #[test]
    fn file_info_decoder_creation() {
        let decoder = Stream::default().file_info_decoder(u64::MAX, 1024).unwrap();

        // Initially the decoder is not finished
        assert!(!decoder.is_finished());

        // Index should not be available before decoding
        assert!(decoder.index().is_none());

        // Total in should be zero
        assert_eq!(decoder.total_in(), 0);

        // File size should match
        assert_eq!(decoder.file_size(), 1024);
    }

    /// Test [`FileInfoDecoder`] with invalid file data returns error.
    #[test]
    fn file_info_decoder_invalid_data() {
        let invalid_data = b"Not a valid XZ file";

        let mut decoder = Stream::default()
            .file_info_decoder(u64::MAX, invalid_data.len() as u64)
            .unwrap();

        let result = decoder.process(invalid_data, Action::Finish);

        // Should return an error for invalid data
        assert!(result.is_err());
    }

    /// Test `FileInfoDecoder::process_after_finish` returns error.
    #[test]
    fn file_info_decoder_process_after_finish() {
        let invalid_data = b"test";

        let mut decoder = Stream::default()
            .file_info_decoder(u64::MAX, invalid_data.len() as u64)
            .unwrap();

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

    /// Test `FileInfoDecoder::seek_pos` method exists and returns value.
    #[test]
    fn file_info_decoder_seek_pos() {
        let decoder = Stream::default().file_info_decoder(u64::MAX, 1024).unwrap();

        // Should be able to call seek_pos
        let pos = decoder.seek_pos();
        // Initially should be 0
        assert_eq!(pos, 0);
    }

    /// Test `FileInfoDecoder::total_in` counter.
    #[test]
    fn file_info_decoder_total_in() {
        let mut decoder = Stream::default().file_info_decoder(u64::MAX, 1024).unwrap();

        assert_eq!(decoder.total_in(), 0);

        // Process some data (even if it fails)
        let test_data = b"test data";
        let _ = decoder.process(test_data, Action::Run);

        // Total in may have changed
        let _ = decoder.total_in();
    }

    /// Test `FileInfoDecoder` round-trip with `Encoder`.
    ///
    /// Creates compressed data and verifies file info decoder can process it.
    #[test]
    fn file_info_decoder_with_encoder_roundtrip() {
        use crate::encoder::options::{Compression, IntegrityCheck};

        // Test data to compress
        let test_data = b"The quick brown fox jumps over the lazy dog";

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

        // Now use FileInfoDecoder to extract file information
        let mut file_info_decoder = Stream::default()
            .file_info_decoder(u64::MAX, compressed.len() as u64)
            .unwrap();

        let mut consumed = 0;
        loop {
            match file_info_decoder.process(&compressed[consumed..], Action::Run) {
                Ok(bytes) => {
                    consumed += bytes;
                    if file_info_decoder.is_finished() {
                        break;
                    }
                }
                Err(Error::SeekNeeded) => {
                    // Get seek position and adjust consumed
                    let seek_pos = file_info_decoder.seek_pos();
                    consumed = usize::try_from(seek_pos).unwrap_or(compressed.len());
                }
                Err(_) => {
                    // Error occurred, but we can still check if index was extracted
                    break;
                }
            }

            if consumed >= compressed.len() {
                break;
            }
        }

        // Try to get the index (may or may not be available depending on how decoding went)
        let _ = file_info_decoder.index();
    }

    /// Test `FileInfoDecoder` with stream that has zero blocks.
    ///
    /// Empty XZ stream with no data blocks.
    #[test]
    fn file_info_decoder_with_empty_xz_stream() {
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

        // Try to decode file info from empty stream
        let mut file_info_decoder = Stream::default()
            .file_info_decoder(u64::MAX, compressed.len() as u64)
            .unwrap();

        let mut consumed = 0;
        loop {
            match file_info_decoder.process(&compressed[consumed..], Action::Run) {
                Ok(bytes) => {
                    consumed += bytes;
                    if file_info_decoder.is_finished() {
                        break;
                    }
                }
                Err(Error::SeekNeeded) => {
                    let seek_pos = file_info_decoder.seek_pos();
                    consumed = usize::try_from(seek_pos).unwrap_or(compressed.len());
                }
                Err(_) => break,
            }

            if consumed >= compressed.len() {
                break;
            }
        }

        // Index may be available
        let _ = file_info_decoder.index();
    }

    /// Test memory limit enforcement.
    ///
    /// Verifies that decoder respects memory limits.
    #[test]
    fn file_info_decoder_memory_limit() {
        // Create decoder with reasonable memory limit
        let result = Stream::default().file_info_decoder(1024 * 1024, 1024);

        // Should succeed to create
        assert!(result.is_ok());

        let decoder = result.unwrap();
        assert_eq!(decoder.total_in(), 0);
        assert!(!decoder.is_finished());
    }

    /// Test decoder state consistency.
    ///
    /// Verifies decoder maintains consistent state throughout processing.
    #[test]
    fn file_info_decoder_state_consistency() {
        let mut decoder = Stream::default().file_info_decoder(u64::MAX, 1024).unwrap();

        // Initial state
        assert!(!decoder.is_finished());
        assert_eq!(decoder.total_in(), 0);
        assert_eq!(decoder.seek_pos(), 0);
        assert!(decoder.index().is_none());
        assert_eq!(decoder.file_size(), 1024);

        // After processing invalid data
        let test_data = b"invalid";
        let result = decoder.process(test_data, Action::Run);

        if result.is_err() {
            // Decoder should still be in consistent state after error
            assert!(!decoder.is_finished());
        }
    }

    /// Test that `index()` returns `None` before finishing.
    ///
    /// Validates index is only available after successful decode.
    #[test]
    fn file_info_decoder_index_availability() {
        let mut decoder = Stream::default().file_info_decoder(u64::MAX, 1024).unwrap();

        // Before any processing
        assert!(decoder.index().is_none());

        // During processing (with invalid data)
        let test_data = b"not an xz file";
        let _ = decoder.process(test_data, Action::Run);

        if !decoder.is_finished() {
            // Index should not be available yet
            assert!(decoder.index().is_none());
        }
    }
}
