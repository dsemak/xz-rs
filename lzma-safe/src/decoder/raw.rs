//! Raw LZMA1 decoder.
//!
//! This decoder processes raw liblzma filter streams without any container metadata.

use crate::encoder::options::{FilterType, Lzma1Options, RawFilters};
use crate::{Action, Error, Result, Stream};

use super::options;

/// Streaming decoder for raw LZMA1 filter input.
pub struct RawDecoder {
    options: options::Options,
    lzma1: Lzma1Options,
    stream: Option<Stream>,
    total_in: u64,
    total_out: u64,
    _filters: RawFilters,
}

impl RawDecoder {
    /// Creates a new raw LZMA1 decoder.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::OptionsError`] if the linked liblzma rejects the filter chain.
    pub fn new_lzma1(
        memlimit: u64,
        flags: options::Flags,
        lzma1: Lzma1Options,
        mut stream: Stream,
    ) -> Result<Self> {
        let options = options::Options {
            memlimit,
            flags,
            ..Default::default()
        };
        let filters = crate::encoder::options::prepare_lzma1_filters(&lzma1, FilterType::Lzma1);
        crate::ffi::lzma_raw_decoder(&filters, &mut stream)?;

        Ok(Self {
            options,
            lzma1,
            stream: Some(stream),
            total_in: 0,
            total_out: 0,
            _filters: filters,
        })
    }

    /// Decompresses input data using the raw decoder.
    pub fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        action: Action,
    ) -> Result<(usize, usize)> {
        let Some(mut stream) = self.stream.take() else {
            return Err(Error::ProgError);
        };

        if !input.is_empty() {
            stream.set_next_input(input);
        } else if action == Action::Finish && stream.avail_in() == 0 {
            stream.set_next_input(&[]);
        }
        stream.set_next_out(output);

        let input_before = stream.avail_in();
        let output_before = stream.avail_out();

        let mut result = crate::ffi::lzma_code(&mut stream, action);
        let mut bytes_read = input_before - stream.avail_in();
        let mut bytes_written = output_before - stream.avail_out();

        if matches!(result, Err(Error::BufError)) && (bytes_read != 0 || bytes_written != 0) {
            result = Ok(());
        }

        if action == Action::Finish && bytes_read == 0 && bytes_written == 0 {
            const MAX_RETRIES: usize = 2;

            for _ in 0..MAX_RETRIES {
                if !matches!(result, Ok(()) | Err(Error::BufError)) {
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
                    Err(Error::StreamEnd) => {
                        result = Err(Error::StreamEnd);
                        break;
                    }
                    Ok(()) | Err(Error::BufError) if stream.total_in() == 0 => {
                        if read_delta == 0 && written_delta == 0 {
                            result = Err(Error::StreamEnd);
                            break;
                        }
                        result = next;
                    }
                    _ => {
                        result = next;
                        if read_delta != 0 || written_delta != 0 {
                            break;
                        }
                    }
                }
            }
        }

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

    /// Access to the LZMA1 filter options used by this decoder.
    pub fn lzma1_options(&self) -> &Lzma1Options {
        &self.lzma1
    }
}

impl Drop for RawDecoder {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            stream.finish();
        }
    }
}

// SAFETY: Like the other stream wrappers, this type owns an independent `lzma_stream`.
unsafe impl Send for RawDecoder {}
