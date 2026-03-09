//! Raw LZMA1 encoder.
//!
//! This encoder targets raw liblzma filter streams without any container header or footer.

use crate::encoder::options::{FilterType, Lzma1Options, RawFilters};
use crate::{Action, Error, Result, Stream};

/// Streaming encoder for raw LZMA1 filter output.
pub struct RawEncoder {
    options: Lzma1Options,
    stream: Option<Stream>,
    total_in: u64,
    total_out: u64,
    _filters: RawFilters,
}

impl RawEncoder {
    /// Creates a new raw LZMA1 encoder with the specified options.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::OptionsError`] if the linked liblzma rejects the filter chain.
    pub fn new_lzma1(options: Lzma1Options, mut stream: Stream) -> Result<Self> {
        let filters = crate::encoder::options::prepare_lzma1_filters(&options, FilterType::Lzma1);
        crate::ffi::lzma_raw_encoder(&filters, &mut stream)?;

        Ok(Self {
            options,
            stream: Some(stream),
            total_in: 0,
            total_out: 0,
            _filters: filters,
        })
    }

    /// Processes input data through the raw encoder.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::ProgError`] if unsupported actions are used.
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

impl Drop for RawEncoder {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            stream.finish();
        }
    }
}

// SAFETY: Like the other stream wrappers, this type owns an independent `lzma_stream`.
unsafe impl Send for RawEncoder {}
