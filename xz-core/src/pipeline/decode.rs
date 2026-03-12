//! Shared decoder state machine used by sync and async pipelines.

use lzma_safe::Action;

use crate::buffer::Buffer;
use crate::config::StreamSummary;
use crate::error::{BackendError, Error, Result};
use crate::options::{BuiltDecoder, DecompressionOptions, Flags};

const LZIP_MAGIC: [u8; 4] = *b"LZIP";

/// Describes how the next read should populate the input buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadMode {
    Replace,
    Append,
}

/// Describes what the wrapper should do after a read operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadAction {
    Run,
    Finish,
}

/// Describes what the wrapper should do after advancing the decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunAction {
    Continue,
    Read(ReadMode),
    Finished,
}

/// Result of one decoder state-machine step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunOutcome {
    pub written: usize,
    pub action: RunAction,
}

impl RunOutcome {
    fn new(written: usize, action: RunAction) -> Self {
        Self { written, action }
    }
}

/// Shared decoder session state.
pub struct DecoderSession {
    decoder: Option<BuiltDecoder>,
    input: Vec<u8>,
    output: Buffer,
    pending_len: usize,
    consumed: usize,
    total_in: u64,
    total_out: u64,
    detected_lzip_input: bool,
    lzip_decoder_options: Option<DecompressionOptions>,
    bootstrapped: bool,
}

impl DecoderSession {
    /// Creates a new decoder session with allocated input/output buffers.
    pub fn new(options: &DecompressionOptions) -> Result<Self> {
        Ok(Self {
            decoder: Some(options.build_decoder()?),
            input: vec![0u8; options.input_capacity()],
            output: Buffer::new(options.output_capacity())?,
            pending_len: 0,
            consumed: 0,
            total_in: 0,
            total_out: 0,
            detected_lzip_input: false,
            lzip_decoder_options: None,
            bootstrapped: false,
        })
    }

    /// Advances the decoder until it produces output, needs more input, or finishes.
    pub fn run(&mut self, options: &DecompressionOptions) -> Result<RunOutcome> {
        loop {
            if self.pending_len == 0 {
                self.consumed = 0;
                return Ok(RunOutcome::new(0, RunAction::Read(ReadMode::Replace)));
            }

            if self.consumed >= self.pending_len {
                self.pending_len = 0;
                self.consumed = 0;
                return Ok(RunOutcome::new(0, RunAction::Read(ReadMode::Replace)));
            }

            let consumed = self.consumed;
            let pending_len = self.pending_len;
            let (used, written) = {
                let decoder = match self.decoder.as_mut() {
                    Some(decoder) => decoder,
                    None => unreachable!("decoder session always retains a decoder"),
                };
                let input = &self.input[consumed..pending_len];
                let output = &mut self.output;
                decoder.process(input, output, Action::Run)?
            };

            self.consumed += used;
            self.total_in += used as u64;

            let is_finished = match self.decoder.as_ref() {
                Some(decoder) => decoder.is_finished(),
                None => unreachable!("decoder session always retains a decoder"),
            };
            if is_finished {
                let next_bytes = &self.input[self.consumed..self.pending_len];
                if should_stop_after_stream_end(options, self.detected_lzip_input, next_bytes) {
                    self.pending_len = 0;
                    self.consumed = 0;
                    return Ok(RunOutcome::new(written, RunAction::Finished));
                }

                self.rebuild_for_next_member(options)?;
                let action = if self.pending_len == 0 {
                    RunAction::Read(ReadMode::Replace)
                } else {
                    RunAction::Continue
                };
                if written > 0 {
                    return Ok(RunOutcome::new(written, action));
                }
                continue;
            }

            if written > 0 {
                return Ok(RunOutcome::new(written, RunAction::Continue));
            }

            if used == 0 {
                self.prepare_append_window();
                return Ok(RunOutcome::new(0, RunAction::Read(ReadMode::Append)));
            }
        }
    }

    /// Returns the mutable slice that should be filled by the next read.
    pub fn read_buffer_mut(
        &mut self,
        options: &DecompressionOptions,
        mode: ReadMode,
    ) -> Result<&mut [u8]> {
        match mode {
            ReadMode::Replace => Ok(&mut self.input),
            ReadMode::Append => {
                self.ensure_append_capacity(options)?;
                Ok(&mut self.input[self.pending_len..])
            }
        }
    }

    /// Commits newly read bytes and reports the next orchestration step.
    pub fn commit_read(
        &mut self,
        options: &DecompressionOptions,
        mode: ReadMode,
        read: usize,
    ) -> Result<ReadAction> {
        if read == 0 {
            if mode == ReadMode::Replace && self.total_in == 0 {
                return Err(BackendError::DataError.into());
            }
            return Ok(ReadAction::Finish);
        }

        match mode {
            ReadMode::Replace => {
                self.pending_len = read;
                self.consumed = 0;
                self.bootstrap_if_needed(options)?;
            }
            ReadMode::Append => {
                self.pending_len += read;
                self.consumed = 0;
            }
        }

        Ok(ReadAction::Run)
    }

    /// Returns the current pending input bytes.
    pub fn pending_bytes(&self) -> &[u8] {
        &self.input[..self.pending_len]
    }

    /// Returns the output bytes produced by the last decoder step.
    pub fn output_chunk(&self, written: usize) -> &[u8] {
        &self.output[..written]
    }

    /// Records bytes successfully written by the wrapper.
    pub fn record_output(&mut self, written: usize) {
        self.total_out += written as u64;
    }

    /// Returns the mutable parts needed by the finish path.
    pub fn finish_parts(&mut self) -> (&mut BuiltDecoder, &mut [u8], &mut u64) {
        let decoder = match self.decoder.as_mut() {
            Some(decoder) => decoder,
            None => unreachable!("decoder session always retains a decoder"),
        };
        let output = &mut self.output[..];
        let total_out = &mut self.total_out;
        (decoder, output, total_out)
    }

    /// Builds the final stream summary.
    pub fn summary(&self) -> StreamSummary {
        StreamSummary::new(self.total_in, self.total_out)
    }

    fn bootstrap_if_needed(&mut self, options: &DecompressionOptions) -> Result<()> {
        if self.bootstrapped {
            return Ok(());
        }

        let decoder = match self.decoder.take() {
            Some(decoder) => decoder,
            None => unreachable!("decoder session always retains a decoder"),
        };
        let bootstrap = DecoderBootstrap::new(decoder, options, &self.input[..self.pending_len])?;
        self.decoder = Some(bootstrap.decoder);
        self.detected_lzip_input = bootstrap.detected_lzip_input;
        self.lzip_decoder_options = bootstrap.lzip_decoder_options;
        self.bootstrapped = true;
        Ok(())
    }

    fn prepare_append_window(&mut self) {
        self.pending_len =
            shift_unconsumed_to_front(&mut self.input, self.consumed, self.pending_len);
        self.consumed = 0;
    }

    fn ensure_append_capacity(&mut self, options: &DecompressionOptions) -> Result<()> {
        if self.pending_len < self.input.len() {
            return Ok(());
        }

        let grow_by = options.input_capacity().max(1);
        self.input
            .try_reserve(grow_by)
            .map_err(|_| Error::AllocationFailed {
                capacity: self.input.len() + grow_by,
            })?;
        self.input.resize(self.input.len() + grow_by, 0);
        Ok(())
    }

    fn rebuild_for_next_member(&mut self, options: &DecompressionOptions) -> Result<()> {
        self.decoder = Some(rebuild_decoder_for_next_member(
            options,
            self.lzip_decoder_options.as_ref(),
        )?);
        if self.pending_len > self.consumed {
            self.pending_len =
                shift_unconsumed_to_front(&mut self.input, self.consumed, self.pending_len);
        } else {
            self.pending_len = 0;
        }
        self.consumed = 0;
        Ok(())
    }
}

/// Decoder state derived from the first input chunk.
struct DecoderBootstrap {
    decoder: BuiltDecoder,
    detected_lzip_input: bool,
    lzip_decoder_options: Option<DecompressionOptions>,
}

impl DecoderBootstrap {
    /// Builds first-chunk decoder state.
    fn new(
        decoder: BuiltDecoder,
        options: &DecompressionOptions,
        first_chunk: &[u8],
    ) -> Result<Self> {
        let detected_lzip_input = first_chunk.starts_with(&LZIP_MAGIC);
        if detected_lzip_input && options.flags().is_concatenated() {
            let mut lzip_flags = options.flags();
            lzip_flags.remove(Flags::CONCATENATED);
            let lzip_options = options.clone().with_flags(lzip_flags);
            return Ok(Self {
                decoder: lzip_options.build_decoder()?,
                detected_lzip_input,
                lzip_decoder_options: Some(lzip_options),
            });
        }

        Ok(Self {
            decoder,
            detected_lzip_input,
            lzip_decoder_options: None,
        })
    }
}

/// Builds a new decoder for the next concatenated stream/member.
fn rebuild_decoder_for_next_member(
    options: &DecompressionOptions,
    lzip_decoder_options: Option<&DecompressionOptions>,
) -> Result<BuiltDecoder> {
    if let Some(lzip_options) = lzip_decoder_options {
        return lzip_options.build_decoder();
    }
    options.build_decoder()
}

/// Moves the unconsumed input tail to the beginning of the buffer.
fn shift_unconsumed_to_front(input: &mut [u8], consumed: usize, pending_len: usize) -> usize {
    let remaining = pending_len - consumed;
    if remaining > 0 {
        input.copy_within(consumed..pending_len, 0);
    }
    remaining
}

/// Returns `true` when stream decoding should stop successfully after `StreamEnd`.
pub fn should_stop_after_stream_end(
    options: &DecompressionOptions,
    detected_lzip_input: bool,
    next_bytes: &[u8],
) -> bool {
    if !options.flags().is_concatenated() {
        return true;
    }

    detected_lzip_input && !next_bytes.is_empty() && !next_bytes.starts_with(&LZIP_MAGIC)
}
