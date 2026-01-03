//! Legacy `.lzma` (`LZMA_Alone`) encoder.
//!
//! This encoder targets the historical `.lzma` file format. It exists for compatibility with
//! legacy tools and data sets.
//!
//! The format supports only LZMA1 and does not include an integrity check field. As a result,
//! only [`crate::Action::Run`] and [`crate::Action::Finish`] are supported.

use crate::encoder::options::Lzma1Options;
use crate::{Action, Error, Result, Stream};

/// Streaming encoder for the legacy `.lzma` (`LZMA_Alone`) container format.
///
/// This is a thin safe wrapper around `lzma_alone_encoder()` + `lzma_code()`.
pub struct AloneEncoder {
    options: Lzma1Options,
    stream: Option<Stream>,
    total_in: u64,
    total_out: u64,
}

impl AloneEncoder {
    /// Create a new `.lzma` encoder with the specified LZMA1 options.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::OptionsError`] if the options are invalid for the linked liblzma.
    pub fn new(options: Lzma1Options, mut stream: Stream) -> Result<Self> {
        crate::ffi::lzma_alone_encoder(options.as_raw(), &mut stream)?;
        Ok(Self {
            options,
            stream: Some(stream),
            total_in: 0,
            total_out: 0,
        })
    }

    /// Process input data through the encoder, producing `.lzma` output.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::ProgError`] if `action` isn't supported by the `.lzma` container.
    pub fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        action: Action,
    ) -> Result<(usize, usize)> {
        if !matches!(action, Action::Run | Action::Finish) {
            return Err(Error::ProgError);
        }

        let Some(mut stream) = self.stream.take() else {
            if action == Action::Finish {
                return Err(Error::ProgError);
            }
            return Ok((0, 0));
        };

        if !input.is_empty() {
            stream.set_next_input(input);
        }
        stream.set_next_out(output);

        let input_before = stream.avail_in();
        let output_before = stream.avail_out();

        let result = crate::ffi::lzma_code(&mut stream, action);
        let bytes_read = input_before - stream.avail_in();
        let bytes_written = output_before - stream.avail_out();

        self.total_in = stream.total_in();
        self.total_out = stream.total_out();

        match result {
            Ok(()) => {
                self.stream = Some(stream);
                Ok((bytes_read, bytes_written))
            }
            Err(Error::StreamEnd) => {
                stream.finish();
                Ok((bytes_read, bytes_written))
            }
            Err(err) => {
                self.stream = Some(stream);
                Err(err)
            }
        }
    }

    /// Whether the underlying stream has been closed.
    pub fn is_finished(&self) -> bool {
        self.stream.is_none()
    }

    /// Total number of input bytes consumed.
    pub fn total_in(&self) -> u64 {
        self.total_in
    }

    /// Total number of output bytes emitted.
    pub fn total_out(&self) -> u64 {
        self.total_out
    }

    /// Access to the LZMA1 options used by this encoder.
    pub fn options(&self) -> &Lzma1Options {
        &self.options
    }
}

impl Drop for AloneEncoder {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            stream.finish();
        }
    }
}

// SAFETY: Like `Encoder`, this type owns an independent `lzma_stream`.
unsafe impl Send for AloneEncoder {}
