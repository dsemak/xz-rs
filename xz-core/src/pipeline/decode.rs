//! Shared decoder state machine used by sync and async pipelines.

use std::io::{self, Read};

use lzma_safe::Action;

use crate::buffer::Buffer;
use crate::config::{
    DecodeMode, DecompressionOutcome, DecompressionStatus, StreamSummary, UnknownInputPolicy,
};
use crate::error::{BackendError, Error, Result};
use crate::header::{
    detect_unsupported_xz_check_id, is_known_decode_format, read_decode_format_probe_prefix,
    LZIP_HEADER_MAGIC,
};
use crate::options::{BuiltDecoder, DecompressionOptions, Flags};

/// Size of the I/O buffer used by the decoder during passthrough.
const IO_BUFFER_SIZE: usize = 8192;

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

/// Probe result captured before starting decompression.
pub struct DecompressionProbe {
    prefix: Vec<u8>,
    status: DecompressionStatus,
    unsupported_check_id: Option<u32>,
}

impl DecompressionProbe {
    /// Probe a synchronous reader before creating the decode stream.
    pub fn read_sync<R: Read>(reader: &mut R, options: &DecompressionOptions) -> io::Result<Self> {
        if options.mode() == DecodeMode::Raw {
            return Ok(Self::decoded(Vec::new(), None));
        }

        let prefix = read_decode_format_probe_prefix(reader)?;
        Ok(Self::classify(prefix, options))
    }

    /// Returns `true` if the pipeline should passthrough the input.
    pub fn is_passthrough(&self) -> bool {
        self.status == DecompressionStatus::Passthrough
    }

    /// Returns the preserved probe prefix.
    pub fn prefix(&self) -> &[u8] {
        &self.prefix
    }

    /// Builds the final decompression outcome from a stream summary.
    pub fn build_outcome(&self, summary: StreamSummary) -> DecompressionOutcome {
        DecompressionOutcome::new(summary, self.status, self.unsupported_check_id)
    }

    fn decoded(prefix: Vec<u8>, unsupported_check_id: Option<u32>) -> Self {
        Self {
            prefix,
            status: DecompressionStatus::Decompressed,
            unsupported_check_id,
        }
    }

    fn classify(prefix: Vec<u8>, options: &DecompressionOptions) -> Self {
        let unsupported_check_id = detect_unsupported_xz_check_id(&prefix);
        let should_passthrough = options.mode() == DecodeMode::Auto
            && options.unknown_input_policy() == UnknownInputPolicy::Passthrough
            && !prefix.is_empty()
            && !is_known_decode_format(&prefix);

        if should_passthrough {
            Self {
                prefix,
                status: DecompressionStatus::Passthrough,
                unsupported_check_id: None,
            }
        } else {
            Self::decoded(prefix, unsupported_check_id)
        }
    }
}

/// Copy already-read prefix and the remaining reader contents to the output unchanged.
pub fn passthrough_sync<R: Read, W: io::Write>(
    prefix: &[u8],
    reader: &mut R,
    writer: &mut W,
) -> io::Result<StreamSummary> {
    let mut bytes_read = 0_u64;
    let mut bytes_written = 0_u64;

    if !prefix.is_empty() {
        writer.write_all(prefix)?;
        let prefix_len = prefix.len() as u64;
        bytes_read += prefix_len;
        bytes_written += prefix_len;
    }

    let mut buffer = [0_u8; IO_BUFFER_SIZE];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        writer.write_all(&buffer[..read])?;
        bytes_read += read as u64;
        bytes_written += read as u64;
    }

    writer.flush()?;
    Ok(StreamSummary::new(bytes_read, bytes_written))
}

#[cfg(feature = "async")]
/// Async reader wrapper that drains a prefetched prefix before polling the inner reader.
pub struct PrefixedAsyncReader<R> {
    prefix: Vec<u8>,
    prefix_pos: usize,
    inner: R,
}

#[cfg(feature = "async")]
impl<R> PrefixedAsyncReader<R> {
    /// Creates a new async reader wrapper with a preserved prefix.
    pub fn new(prefix: Vec<u8>, inner: R) -> Self {
        Self {
            prefix,
            prefix_pos: 0,
            inner,
        }
    }
}

#[cfg(feature = "async")]
impl<R> tokio::io::AsyncRead for PrefixedAsyncReader<R>
where
    R: tokio::io::AsyncRead + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        if self.prefix_pos < self.prefix.len() {
            let remaining = self.prefix.len() - self.prefix_pos;
            let to_copy = remaining.min(buf.remaining());
            let end = self.prefix_pos + to_copy;
            buf.put_slice(&self.prefix[self.prefix_pos..end]);
            self.prefix_pos = end;
            return std::task::Poll::Ready(Ok(()));
        }

        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

#[cfg(feature = "async")]
/// Probe an async reader before starting decompression.
pub async fn probe_async<R>(
    reader: &mut R,
    options: &DecompressionOptions,
) -> io::Result<DecompressionProbe>
where
    R: tokio::io::AsyncRead + Unpin,
{
    if options.mode() == DecodeMode::Raw {
        return Ok(DecompressionProbe::decoded(Vec::new(), None));
    }

    let mut prefix = Vec::with_capacity(crate::header::DECODE_FORMAT_PROBE_SIZE);
    let mut buffer = [0_u8; crate::header::DECODE_FORMAT_PROBE_SIZE];

    while prefix.len() < buffer.len() {
        let offset = prefix.len();
        let read = tokio::io::AsyncReadExt::read(reader, &mut buffer[offset..]).await?;
        if read == 0 {
            break;
        }
        prefix.extend_from_slice(&buffer[offset..offset + read]);
    }

    Ok(DecompressionProbe::classify(prefix, options))
}

#[cfg(feature = "async")]
/// Copy already-read prefix and the remaining async reader contents to the output unchanged.
pub async fn passthrough_async<R, W>(
    prefix: &[u8],
    reader: &mut R,
    writer: &mut W,
) -> io::Result<StreamSummary>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut bytes_read = 0_u64;
    let mut bytes_written = 0_u64;

    if !prefix.is_empty() {
        tokio::io::AsyncWriteExt::write_all(writer, prefix).await?;
        let prefix_len = prefix.len() as u64;
        bytes_read += prefix_len;
        bytes_written += prefix_len;
    }

    let mut buffer = [0_u8; IO_BUFFER_SIZE];
    loop {
        let read = tokio::io::AsyncReadExt::read(reader, &mut buffer).await?;
        if read == 0 {
            break;
        }

        tokio::io::AsyncWriteExt::write_all(writer, &buffer[..read]).await?;
        bytes_read += read as u64;
        bytes_written += read as u64;
    }

    tokio::io::AsyncWriteExt::flush(writer).await?;
    Ok(StreamSummary::new(bytes_read, bytes_written))
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
        let detected_lzip_input = first_chunk.starts_with(&LZIP_HEADER_MAGIC);
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

    detected_lzip_input && !next_bytes.is_empty() && !next_bytes.starts_with(&LZIP_HEADER_MAGIC)
}
