//! High-level encoder built on top of `liblzma`.

use crate::{Action, Result, Stream};

pub mod options;
#[cfg(test)]
mod tests;

pub use options::Options;

/// Safe wrapper around an `lzma_stream` configured for compression.
pub struct Encoder {
    /// Encoder configuration options (compression level, check, threads, etc.).
    options: Options,
    /// Underlying LZMA stream. `None` if encoding is finished or stream is dropped.
    stream: Option<Stream>,
    /// Total number of bytes read from input so far.
    total_in: u64,
    /// Total number of bytes written to output so far.
    total_out: u64,
    /// Keeps filter option buffers alive for the duration of the encoder (used for MT).
    _prepared_filters: Option<options::RawFilters>,
}

impl Encoder {
    /// Creates a new single-threaded encoder with the given compression level and integrity check.
    ///
    /// # Parameters
    ///
    /// * `level` - Compression level or preset to use (see [`options::Compression`]).
    /// * `check` - Integrity check type (see [`options::IntegrityCheck`]).
    /// * `stream` - An initialized [`Stream`] for LZMA operations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OptionsError`] if the encoder options are invalid.
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::MemLimitError`] if the memory limit is exceeded.
    /// Returns [`Error::UnsupportedCheck`] if the integrity check type is not supported.
    /// Returns [`Error::ProgError`] if the encoder is misused.
    ///
    /// # Returns
    ///
    /// Returns the new encoder if successful.
    pub fn new(
        level: options::Compression,
        check: options::IntegrityCheck,
        mut stream: Stream,
    ) -> Result<Self> {
        let options = Options {
            level,
            check,
            ..Default::default()
        };

        crate::ffi::lzma_easy_encoder(options.level, options.check, &mut stream)?;

        Ok(Encoder {
            options,
            stream: Some(stream),
            total_in: 0,
            total_out: 0,
            _prepared_filters: None,
        })
    }

    /// Creates a new multi-threaded encoder with the specified options.
    ///
    /// # Parameters
    ///
    /// * `options` - Encoder configuration (see [`Options`]).
    /// * `stream` - An initialized [`Stream`] for LZMA operations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OptionsError`] if the encoder options are invalid.
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::MemLimitError`] if the memory limit is exceeded.
    /// Returns [`Error::UnsupportedCheck`] if the integrity check type is not supported.
    /// Returns [`Error::ProgError`] if the encoder is misused.
    ///
    /// # Returns
    ///
    /// Returns the new encoder if successful.
    pub fn new_mt(options: Options, mut stream: Stream) -> Result<Self> {
        let prepared_filters = crate::ffi::lzma_stream_encoder_mt(&options, &mut stream)?;

        Ok(Encoder {
            options,
            stream: Some(stream),
            total_in: 0,
            total_out: 0,
            _prepared_filters: prepared_filters,
        })
    }

    /// Processes input data through the encoder, producing compressed output.
    ///
    /// # Parameters
    ///
    /// * `input` - Input data to compress.
    /// * `output` - Output buffer for compressed data.
    /// * `action` - Action to perform (e.g., [`Action::Run`], [`Action::Finish`]).
    ///
    /// # Errors
    ///
    /// Returns [`Error::BufError`] if no progress is possible (e.g., output buffer too small).
    /// Returns [`Error::DataError`] if input data is corrupted.
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::ProgError`] if the encoder is misused (e.g., trying to finish twice).
    ///
    /// # Returns
    ///
    /// Returns a tuple `(bytes_read, bytes_written)` indicating how many bytes were consumed from
    /// the input and how many bytes were written to the output. If the stream is finished, returns
    /// `(0, 0)` for most actions or an error for `Action::Finish`.
    pub fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        action: Action,
    ) -> Result<(usize, usize)> {
        // If the stream is already finished, prevent further processing.
        let Some(mut stream) = self.stream.take() else {
            if action == Action::Finish {
                // Cannot finish an already finished stream.
                return Err(crate::Error::ProgError);
            }
            // For other actions, allow querying but no more data will be processed.
            return Ok((0, 0));
        };

        // Provide new input only when data is available; otherwise preserve any
        // buffered bytes that liblzma still needs to consume.
        if !input.is_empty() {
            stream.set_next_input(input);
        }

        // Set up output buffer pointers and lengths for the FFI call.
        stream.set_next_out(output);

        let input_before = stream.avail_in();
        let output_before = stream.avail_out();

        let result = crate::ffi::lzma_code(&mut stream, action);
        let bytes_read = input_before - stream.avail_in();
        let bytes_written = output_before - stream.avail_out();

        // Update total bytes processed.
        self.total_in = stream.total_in();
        self.total_out = stream.total_out();

        match result {
            Ok(()) => {
                // Encoding succeeded, keep the stream for further use.
                self.stream = Some(stream);
                Ok((bytes_read, bytes_written))
            }
            Err(crate::Error::StreamEnd) => {
                // The stream has ended; mark as finished and return the last processed bytes.
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

    /// Number of worker threads configured for compression.
    pub fn threads(&self) -> u32 {
        self.options.threads
    }

    /// The compression preset passed to the encoder.
    pub fn compression_level(&self) -> options::Compression {
        self.options.level
    }

    /// Which integrity check will be stored in the output stream.
    pub fn check(&self) -> options::IntegrityCheck {
        self.options.check
    }

    /// Total number of input bytes consumed.
    pub fn total_in(&self) -> u64 {
        self.total_in
    }

    /// Total number of output bytes emitted.
    pub fn total_out(&self) -> u64 {
        self.total_out
    }
}

impl Drop for Encoder {
    /// Ensures the underlying stream is finalized and resources are released.
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            stream.finish();
        }
    }
}

// SAFETY: Encoder is Send because the underlying lzma_stream is not shared across threads.
// liblzma streams are not thread-safe for concurrent access, so Sync is not implemented.
unsafe impl Send for Encoder {}
