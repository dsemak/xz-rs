//! Builder for encoder/decoder instances backed by `liblzma`.

use std::ptr;
use std::sync::Arc;

mod allocator;
mod index;
#[cfg(test)]
mod tests;

pub use allocator::{Allocator, LzmaAllocator, StdAllocator};
pub use index::{
    BlockInfo, Index, IndexEntry, IndexIterMode, IndexIterator, StreamFlags, StreamInfo,
};

use crate::decoder;
use crate::encoder;
use crate::{Decoder, Encoder, FileInfoDecoder, IndexDecoder, Result};

/// Safe wrapper around `lzma_stream` with optional custom allocator.
pub struct Stream {
    /// The raw `lzma_stream` struct from liblzma.
    inner: liblzma_sys::lzma_stream,
    /// Optional custom allocator, kept alive for the stream's lifetime.
    #[allow(unused)]
    allocator: Option<LzmaAllocator>,
}

impl Default for Stream {
    fn default() -> Self {
        Self::with_allocator(None)
    }
}

impl Stream {
    /// Create a new `Stream` with an optional custom allocator.
    pub fn with_allocator(allocator: Option<Arc<dyn Allocator>>) -> Self {
        let allocator = allocator.map(LzmaAllocator::from_allocator);
        let allocator_ptr = allocator
            .as_ref()
            .map_or(std::ptr::null(), allocator::LzmaAllocator::as_ptr);

        // Initialize the lzma_stream struct with all fields set to zero or null,
        // except for the allocator pointer and reserved enums.
        let inner = liblzma_sys::lzma_stream {
            next_in: ptr::null(),
            avail_in: 0,
            total_in: 0,
            next_out: ptr::null_mut(),
            avail_out: 0,
            total_out: 0,
            allocator: allocator_ptr,
            internal: ptr::null_mut(),
            reserved_ptr1: ptr::null_mut(),
            reserved_ptr2: ptr::null_mut(),
            reserved_ptr3: ptr::null_mut(),
            reserved_ptr4: ptr::null_mut(),
            seek_pos: 0,
            reserved_int2: 0,
            reserved_int3: 0,
            reserved_int4: 0,
            reserved_enum1: liblzma_sys::lzma_reserved_enum_LZMA_RESERVED_ENUM,
            reserved_enum2: liblzma_sys::lzma_reserved_enum_LZMA_RESERVED_ENUM,
        };

        Self { inner, allocator }
    }

    /// Get a clone of the allocator used by this stream.
    pub(crate) fn allocator(&self) -> Option<LzmaAllocator> {
        self.allocator.clone()
    }

    /// Create an encoder using the "easy" preset interface.
    ///
    /// # Parameters
    ///
    /// * `level` - Compression level or preset (see [`encoder::options::Compression`])
    /// * `check` - Integrity check type (see [`encoder::options::IntegrityCheck`])
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
    /// Returns an [`Encoder`] on success.
    pub fn easy_encoder(
        self,
        level: encoder::options::Compression,
        check: encoder::options::IntegrityCheck,
    ) -> Result<Encoder> {
        Encoder::new(level, check, self)
    }

    /// Create a multithreaded encoder with the specified options.
    ///
    /// # Parameters
    ///
    /// * `level` - Compression level or preset (see [`encoder::options::Compression`])
    /// * `check` - Integrity check type (see [`encoder::options::IntegrityCheck`])
    /// * `threads` - Number of worker threads (minimum 1).
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
    /// Returns an [`Encoder`] on success.
    pub fn multithreaded_encoder(
        self,
        level: encoder::options::Compression,
        check: encoder::options::IntegrityCheck,
        threads: u32,
    ) -> Result<Encoder> {
        // liblzma requires at least one thread; default to 1 if zero is given.
        let threads = if threads == 0 { 1 } else { threads };
        let options = encoder::Options {
            level,
            check,
            threads,
            ..Default::default()
        };

        Encoder::new_mt(options, self)
    }

    /// Create a decoder with the specified memory limit and flags.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for decompression.
    /// * `flags` - Decoder flags.
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
    /// Returns a [`Decoder`] on success.
    pub fn decoder(self, memlimit: u64, flags: decoder::options::Flags) -> Result<Decoder> {
        Decoder::new(memlimit, flags, self)
    }

    /// Create a multithreaded decoder with the specified options.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for decompression.
    /// * `flags` - Decoder flags.
    /// * `threads` - Number of worker threads.
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
    /// Returns a [`Decoder`] on success.
    pub fn mt_decoder(
        self,
        memlimit: u64,
        flags: decoder::options::Flags,
        threads: u32,
    ) -> Result<Decoder> {
        let options = decoder::Options {
            threads,
            memlimit,
            flags,
            ..Default::default()
        };
        Decoder::new_mt(options, self)
    }

    /// Create a decoder that automatically detects the container format.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for decompression.
    /// * `flags` - Decoder flags.
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
    /// Returns a [`Decoder`] on success.
    pub fn auto_decoder(self, memlimit: u64, flags: decoder::options::Flags) -> Result<Decoder> {
        Decoder::new_auto(memlimit, flags, self)
    }

    /// Create a decoder for the legacy `.lzma` format.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for decompression.
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
    /// Returns a [`Decoder`] on success.
    pub fn alone_decoder(self, memlimit: u64) -> Result<Decoder> {
        Decoder::new_alone(memlimit, self)
    }

    /// Create an index decoder for extracting metadata from XZ Index blocks.
    ///
    /// This decoder extracts information about streams, blocks, and other metadata
    /// from XZ Index blocks without decompressing the actual data.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for the decoder.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::ProgError`] if the decoder is misused.
    ///
    /// # Returns
    ///
    /// Returns a [`IndexDecoder`] on success.
    pub fn index_decoder(self, memlimit: u64) -> Result<IndexDecoder> {
        IndexDecoder::new(memlimit, self)
    }

    /// Create a file info decoder for extracting metadata from complete XZ files.
    ///
    /// This decoder reads Stream Headers, Stream Footers, Index blocks, and
    /// Stream Padding to build a combined index of all streams in the file.
    /// It may request the application to seek to different positions in the file.
    ///
    /// # Parameters
    ///
    /// * `memlimit` - Maximum memory usage for the decoder.
    /// * `file_size` - Total size of the input XZ file in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MemError`] if memory allocation fails.
    /// Returns [`Error::ProgError`] if the decoder is misused.
    ///
    /// # Returns
    ///
    /// Returns a [`FileInfoDecoder`] on success.
    pub fn file_info_decoder(self, memlimit: u64, file_size: u64) -> Result<FileInfoDecoder> {
        FileInfoDecoder::new(memlimit, file_size, self)
    }

    /// Internal helper exposing the raw `lzma_stream`.
    pub(crate) fn lzma_stream(&mut self) -> &mut liblzma_sys::lzma_stream {
        &mut self.inner
    }

    /// Finalise the stream by calling into liblzma.
    pub(crate) fn finish(self) {
        crate::ffi::lzma_end(self);
    }

    /// Update the input buffer.
    pub(crate) fn set_next_input(&mut self, input: &[u8]) {
        let next_in = if input.is_empty() {
            std::ptr::null()
        } else {
            input.as_ptr()
        };

        self.inner.next_in = next_in;
        self.inner.avail_in = input.len();
    }

    /// Update the output buffer.
    pub(crate) fn set_next_out(&mut self, output: &mut [u8]) {
        let next_out = if output.is_empty() {
            std::ptr::null_mut()
        } else {
            output.as_mut_ptr()
        };

        self.inner.next_out = next_out;
        self.inner.avail_out = output.len();
    }

    /// Total number of input bytes processed.
    pub(crate) fn total_in(&self) -> u64 {
        self.inner.total_in
    }

    /// Total number of output bytes produced.
    pub(crate) fn total_out(&self) -> u64 {
        self.inner.total_out
    }

    /// Remaining bytes in the current input buffer.
    pub(crate) fn avail_in(&self) -> usize {
        self.inner.avail_in
    }

    /// Remaining space in the current output buffer.
    pub(crate) fn avail_out(&self) -> usize {
        self.inner.avail_out
    }

    /// Get the seek position requested by liblzma.
    ///
    /// This is used by file info decoder when it returns `LZMA_SEEK_NEEDED`.
    pub(crate) fn seek_pos(&self) -> u64 {
        self.inner.seek_pos
    }
}
