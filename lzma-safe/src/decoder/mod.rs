//! High-level decompression helpers built on top of `liblzma`.

use crate::{Action, Result, Stream};

mod file_info;
mod index;
pub mod options;
#[cfg(test)]
mod tests;

pub use file_info::FileInfoDecoder;
pub use index::IndexDecoder;
pub use options::Options;

/// Safe wrapper around an `lzma_stream` configured for decompression.
pub struct Decoder {
    /// Decoder configuration options (threads, memlimit, flags, etc.).
    options: Options,
    /// Underlying LZMA stream. `None` if decoding is finished or stream is dropped.
    stream: Option<Stream>,
    /// Total number of bytes read from input so far.
    total_in: u64,
    /// Total number of bytes written to output so far.
    total_out: u64,
}

impl Decoder {
    /// Creates a new LZMA stream decoder with the given memory limit and flags.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for decoding (in bytes).
    /// * `flags` - Decoder behavior flags (see [`options::Flags`]).
    /// * `stream` - An initialized [`Stream`] for LZMA operations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OptionsError`] if the decoder options are invalid.
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::MemLimitError`] if the memory limit is exceeded.
    /// Returns [`Error::FormatError`] if the input format is not recognized.
    /// Returns [`Error::UnsupportedCheck`] if the integrity check type is not supported.
    /// Returns [`Error::ProgError`] if the decoder is misused.
    ///
    /// # Returns
    /// Returns the new decoder if successful.
    pub fn new(memlimit: u64, flags: options::Flags, mut stream: Stream) -> Result<Self> {
        let options = Options {
            memlimit,
            flags,
            ..Default::default()
        };

        // Initialize the LZMA stream decoder with the specified options.
        crate::ffi::lzma_stream_decoder(options.memlimit, options.flags, &mut stream)?;

        Ok(Decoder {
            options,
            stream: Some(stream),
            total_in: 0,
            total_out: 0,
        })
    }

    /// Creates a new LZMA auto decoder, which can handle both .xz and .lzma formats.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for decoding (in bytes).
    /// * `flags` - Decoder behavior flags (see [`options::Flags`]).
    /// * `stream` - An initialized [`Stream`] for LZMA operations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OptionsError`] if the decoder options are invalid.
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::MemLimitError`] if the memory limit is exceeded.
    /// Returns [`Error::FormatError`] if the input format is not recognized.
    /// Returns [`Error::UnsupportedCheck`] if the integrity check type is not supported.
    /// Returns [`Error::ProgError`] if the decoder is misused.
    ///
    /// # Returns
    ///
    /// Returns the new decoder if successful.
    pub fn new_auto(memlimit: u64, flags: options::Flags, mut stream: Stream) -> Result<Self> {
        let options = Options {
            memlimit,
            flags,
            ..Default::default()
        };

        // Initialize the LZMA auto decoder (supports .xz and .lzma).
        crate::ffi::lzma_auto_decoder(options.memlimit, options.flags, &mut stream)?;

        Ok(Decoder {
            options,
            stream: Some(stream),
            total_in: 0,
            total_out: 0,
        })
    }

    /// Creates a new LZMA "alone" decoder (for legacy .lzma files).
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for decoding (in bytes).
    /// * `stream` - An initialized [`Stream`] for LZMA operations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OptionsError`] if the decoder options are invalid.
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::MemLimitError`] if the memory limit is exceeded.
    /// Returns [`Error::FormatError`] if the input format is not recognized.
    /// Returns [`Error::UnsupportedCheck`] if the integrity check type is not supported.
    /// Returns [`Error::ProgError`] if the decoder is misused.
    ///
    /// # Returns
    /// Returns the new decoder if successful.
    pub fn new_alone(memlimit: u64, mut stream: Stream) -> Result<Self> {
        let options = Options {
            memlimit,
            ..Default::default()
        };

        // Initialize the LZMA "alone" decoder (legacy .lzma format).
        crate::ffi::lzma_alone_decoder(options.memlimit, &mut stream)?;

        Ok(Decoder {
            stream: Some(stream),
            options,
            total_in: 0,
            total_out: 0,
        })
    }

    /// Creates a new multi-threaded LZMA decoder with the specified options.
    ///
    /// # Parameters
    ///
    /// * `options` - Decoder configuration (see [`Options`]).
    /// * `stream` - An initialized [`Stream`] for LZMA operations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OptionsError`] if the decoder options are invalid.
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::MemLimitError`] if the memory limit is exceeded.
    /// Returns [`Error::FormatError`] if the input format is not recognized.
    /// Returns [`Error::UnsupportedCheck`] if the integrity check type is not supported.
    /// Returns [`Error::ProgError`] if the decoder is misused.
    ///
    /// # Returns
    ///
    /// Returns the new decoder if successful.
    pub fn new_mt(options: Options, mut stream: Stream) -> Result<Self> {
        // Initialize the multi-threaded LZMA stream decoder.
        crate::ffi::lzma_stream_decoder_mt(&options, &mut stream)?;

        Ok(Decoder {
            options,
            stream: Some(stream),
            total_in: 0,
            total_out: 0,
        })
    }

    /// Decompresses input data using the decoder.
    ///
    /// # Parameters
    ///
    /// * `input` - Buffer containing compressed data to decode.
    /// * `output` - Buffer to write decompressed data into.
    /// * `action` - Decoding action (e.g., [`Action::Run`], [`Action::Finish`]).
    ///
    /// # Errors
    ///
    /// Returns [`Error::BufError`] if no progress is possible (e.g., output buffer too small).
    /// Returns [`Error::DataError`] if input data is corrupted.
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::ProgError`] if the decoder is misused (e.g., trying to finish twice).
    ///
    /// # Returns
    ///
    /// Returns a tuple `(bytes_read, bytes_written)` on success, indicating how much input was consumed and output produced.
    pub fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        action: Action,
    ) -> Result<(usize, usize)> {
        // Take ownership of the stream for this operation.
        let Some(mut stream) = self.stream.take() else {
            // Stream is already finished or dropped; this is a logic error.
            return Err(crate::Error::ProgError);
        };

        // Only update the input pointer when new data is supplied so that liblzma can continue
        // consuming any buffered bytes from previous calls when `input` is empty.
        //
        // However, when finishing and there are no buffered bytes left, explicitly clear the input
        // pointer (next_in = NULL). Some liblzma paths rely on next_in being NULL at EOF.
        if !input.is_empty() {
            stream.set_next_input(input);
        } else if action == Action::Finish && stream.avail_in() == 0 {
            stream.set_next_input(&[]);
        }
        stream.set_next_out(output);

        let input_before = stream.avail_in();
        let output_before = stream.avail_out();

        // Perform the decompression step.
        let mut result = crate::ffi::lzma_code(&mut stream, action);
        let mut bytes_read = input_before - stream.avail_in();
        let mut bytes_written = output_before - stream.avail_out();

        // liblzma can return `LZMA_BUF_ERROR` even after making progress (e.g. output buffer is
        // full). In that case, treat it as "decoding continues" and let the caller retry with a
        // fresh output buffer (or more input).
        if matches!(result, Err(crate::Error::BufError)) && (bytes_read != 0 || bytes_written != 0)
        {
            result = Ok(());
        }

        // Handle a subtle corner case for Action::Finish with no progress: liblzma may require one
        // or more additional calls to transition to `LZMA_STREAM_END`.
        //
        // IMPORTANT: we never "force" StreamEnd for non-empty inputs. If liblzma cannot reach
        // StreamEnd, the caller should treat this as truncated/corrupt input.
        if action == Action::Finish && bytes_read == 0 && bytes_written == 0 {
            const MAX_RETRIES: usize = 2;

            for _ in 0..MAX_RETRIES {
                if !matches!(result, Ok(()) | Err(crate::Error::BufError)) {
                    break;
                }

                let in_before = stream.avail_in();
                let out_before = stream.avail_out();
                let next = crate::ffi::lzma_code(&mut stream, action);
                let read_delta = in_before - stream.avail_in();
                let written_delta = out_before - stream.avail_out();
                bytes_read += read_delta;
                bytes_written += written_delta;

                match next {
                    Err(crate::Error::StreamEnd) => {
                        result = Err(crate::Error::StreamEnd);
                        break;
                    }
                    // Treat a never-fed decoder as a successful empty stream.
                    Ok(()) | Err(crate::Error::BufError) if stream.total_in() == 0 => {
                        if read_delta == 0 && written_delta == 0 {
                            result = Err(crate::Error::StreamEnd);
                            break;
                        }
                        result = next;
                    }
                    _ => {
                        result = next;
                        // If we made progress, stop retrying.
                        if read_delta != 0 || written_delta != 0 {
                            break;
                        }
                    }
                }
            }
        }

        // Update total counters.
        self.total_in = stream.total_in();
        self.total_out = stream.total_out();

        match result {
            Ok(()) => {
                // Decoding continues; put the stream back for further use.
                self.stream = Some(stream);
                Ok((bytes_read, bytes_written))
            }
            Err(crate::Error::StreamEnd) => {
                // Decoding is finished; finalize the stream and mark as finished.
                stream.finish();
                Ok((bytes_read, bytes_written))
            }
            Err(err) => {
                // On error, retain the stream for possible recovery or inspection.
                self.stream = Some(stream);
                Err(err)
            }
        }
    }

    /// Whether the underlying stream has been closed.
    pub fn is_finished(&self) -> bool {
        self.stream.is_none()
    }

    /// Total number of bytes consumed from the input side.
    pub fn total_in(&self) -> u64 {
        self.total_in
    }

    /// Total number of bytes produced by the decoder.
    pub fn total_out(&self) -> u64 {
        self.total_out
    }

    /// Memory limit passed to the decoder.
    pub fn memlimit(&self) -> u64 {
        self.options.memlimit
    }

    /// Flags configured for decoding.
    pub fn flags(&self) -> options::Flags {
        self.options.flags
    }

    /// Number of worker threads, if multi-threaded decoding is enabled.
    pub fn threads(&self) -> u32 {
        self.options.threads
    }
}

impl Drop for Decoder {
    /// Ensures the underlying stream is finalized and resources are released.
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            stream.finish();
        }
    }
}

// SAFETY: Decoder is Send because the underlying lzma_stream is not shared across threads.
// liblzma streams are not thread-safe for concurrent access, so Sync is not implemented.
unsafe impl Send for Decoder {}
