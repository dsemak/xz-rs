//! Safe RAII wrappers around `lzma_index` and iterators over its contents.

use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;

use crate::encoder::options::IntegrityCheck;
use crate::ffi;
use crate::stream::LzmaAllocator;
use crate::{Error, Result};

/// Owned handle to a liblzma `lzma_index`.
///
/// The index is freed automatically via [`liblzma_sys::lzma_index_end`] when
/// this struct is dropped.
pub struct Index {
    inner: NonNull<liblzma_sys::lzma_index>,
    /// Optional custom allocator, kept alive for the stream's lifetime.
    #[allow(unused)]
    allocator: Option<LzmaAllocator>,
}

impl Index {
    /// Construct an [`Index`] from a raw pointer returned by liblzma.
    ///
    /// # Safety
    ///
    /// * `ptr` must be a valid pointer obtained from liblzma.
    /// * The caller relinquishes ownership of the pointer to the returned
    ///   `Index` instance.
    /// * If `allocator` is provided, it must be the same allocator that was
    ///   used to create the index.
    pub(crate) unsafe fn from_raw(
        ptr: *mut liblzma_sys::lzma_index,
        allocator: Option<LzmaAllocator>,
    ) -> Option<Self> {
        NonNull::new(ptr).map(|inner| Self { inner, allocator })
    }

    /// Expose the raw pointer for FFI calls that only need shared access.
    pub(crate) fn as_ptr(&self) -> *const liblzma_sys::lzma_index {
        self.inner.as_ptr().cast_const()
    }

    /// Expose the raw pointer for FFI calls that need mutable access.
    pub(crate) fn as_mut_ptr(&mut self) -> *mut liblzma_sys::lzma_index {
        self.inner.as_ptr()
    }

    /// Return the number of streams stored in the index.
    pub fn stream_count(&self) -> u64 {
        ffi::lzma_index_stream_count(self)
    }

    /// Return the number of blocks stored in the index.
    pub fn block_count(&self) -> u64 {
        ffi::lzma_index_block_count(self)
    }

    /// Return the total compressed size tracked by the index.
    pub fn file_size(&self) -> u64 {
        ffi::lzma_index_file_size(self)
    }

    /// Return the total uncompressed size tracked by the index.
    pub fn uncompressed_size(&self) -> u64 {
        ffi::lzma_index_uncompressed_size(self)
    }

    /// Return the total size of the Stream represented by this index.
    pub fn stream_size(&self) -> u64 {
        ffi::lzma_index_stream_size(self)
    }

    /// Return the bitmask of integrity checks seen in the index.
    pub fn checks(&self) -> u32 {
        ffi::lzma_index_checks(self)
    }

    /// Decode an XZ Index field (as stored in the Stream) into an [`Index`].
    ///
    /// # Errors
    ///
    /// Returns an error if the Index field is corrupted, truncated, or if
    /// decoding exceeds `memlimit`.
    pub fn decode_xz_index_field(index_field: &[u8], memlimit: u64) -> Result<Self> {
        let mut limit = memlimit;
        let (index, consumed) = ffi::decode_xz_index_field(&mut limit, index_field, None)?;
        if consumed != index_field.len() {
            // The caller should provide exactly the Index field bytes.
            return Err(Error::DataError);
        }
        Ok(index)
    }

    /// Set Stream Flags for the last (and typically the only) Stream in this index.
    ///
    /// This is needed for functions like `checks()` to report meaningful values and for
    /// downstream code that needs to know the integrity check type.
    pub fn set_stream_flags_from_footer(
        &mut self,
        footer: &[u8; crate::stream::HEADER_SIZE],
    ) -> Result<()> {
        let flags = StreamFlags::decode_footer(footer)?;
        ffi::lzma_index_stream_flags(self, &flags)
    }

    /// Set Stream Padding for the last Stream in this index.
    pub fn set_stream_padding(&mut self, padding: u64) -> Result<()> {
        ffi::lzma_index_stream_padding(self, padding)
    }

    /// Append `other` after `self`, concatenating Stream information.
    ///
    /// On success, `other` is consumed and must not be used again.
    pub fn append(&mut self, other: Index) -> Result<()> {
        // Take a local clone to avoid borrowing `self` immutably while it is mutably borrowed.
        let allocator = self.allocator.clone();
        let mut other = ManuallyDrop::new(other);
        match ffi::lzma_index_cat(self, &mut other, allocator.as_ref()) {
            Ok(()) => Ok(()),
            Err(err) => {
                // SAFETY: Concatenation failed, `other` still owns its resources.
                unsafe { ManuallyDrop::drop(&mut other) };
                Err(err)
            }
        }
    }

    /// Create an iterator over items stored in the index.
    ///
    /// By default, iterates in [`IndexIterMode::Any`] mode.
    /// Use [`Index::iter_streams()`] or [`Index::iter_blocks()`] for specific modes.
    pub fn iter(&self) -> IndexIterator<'_> {
        IndexIterator::new(self)
    }

    /// Create an iterator over streams only.
    pub fn iter_streams(&self) -> IndexIterator<'_> {
        IndexIterator::with_mode(self, IndexIterMode::Stream)
    }

    /// Create an iterator over blocks only.
    pub fn iter_blocks(&self) -> IndexIterator<'_> {
        IndexIterator::with_mode(self, IndexIterMode::Block)
    }

    /// Create an iterator over non-empty blocks only.
    pub fn iter_non_empty_blocks(&self) -> IndexIterator<'_> {
        IndexIterator::with_mode(self, IndexIterMode::NonEmptyBlock)
    }
}

impl Drop for Index {
    fn drop(&mut self) {
        ffi::lzma_index_end(self.inner.as_ptr(), self.allocator.as_ref());
    }
}

impl<'a> IntoIterator for &'a Index {
    type Item = IndexEntry;
    type IntoIter = IndexIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Entry returned by [`IndexIterator`].
#[derive(Debug, Clone)]
pub enum IndexEntry {
    /// Stream entry with its information.
    Stream(StreamInfo),
    /// Block entry with its information.
    Block(BlockInfo),
}

/// Iterator over streams and blocks stored in an [`Index`].
pub struct IndexIterator<'a> {
    inner: liblzma_sys::lzma_index_iter,
    mode: IndexIterMode,
    _owner: PhantomData<&'a Index>,
}

impl<'a> IndexIterator<'a> {
    fn new(index: &'a Index) -> Self {
        Self::with_mode(index, IndexIterMode::Any)
    }

    /// Create an iterator with a specific iteration mode.
    pub fn with_mode(index: &'a Index, mode: IndexIterMode) -> Self {
        let mut inner = unsafe { std::mem::zeroed::<liblzma_sys::lzma_index_iter>() };
        // SAFETY: The index pointer is valid and owned by the caller.
        ffi::lzma_index_iter_init(&mut inner, index);

        Self {
            inner,
            mode,
            _owner: PhantomData,
        }
    }

    /// Set the iteration mode for this iterator.
    pub fn set_mode(&mut self, mode: IndexIterMode) {
        self.mode = mode;
    }

    /// Information about the current stream entry.
    pub fn stream(&self) -> StreamInfo {
        // SAFETY: The flags pointer comes from liblzma and is valid for the lifetime of the iterator
        let flags = unsafe { StreamFlags::from_raw(self.inner.stream.flags) };

        StreamInfo {
            number: self.inner.stream.number,
            block_count: self.inner.stream.block_count,
            compressed_offset: self.inner.stream.compressed_offset,
            uncompressed_offset: self.inner.stream.uncompressed_offset,
            compressed_size: self.inner.stream.compressed_size,
            uncompressed_size: self.inner.stream.uncompressed_size,
            padding: self.inner.stream.padding,
            flags,
        }
    }

    /// Information about the current block entry.
    pub fn block(&self) -> BlockInfo {
        BlockInfo {
            number_in_stream: self.inner.block.number_in_stream,
            number_in_file: self.inner.block.number_in_file,
            compressed_file_offset: self.inner.block.compressed_file_offset,
            uncompressed_file_offset: self.inner.block.uncompressed_file_offset,
            total_size: self.inner.block.total_size,
            uncompressed_size: self.inner.block.uncompressed_size,
            unpadded_size: self.inner.block.unpadded_size,
        }
    }

    /// Get the current entry based on the iteration mode.
    fn current_entry(&self) -> IndexEntry {
        match self.mode {
            IndexIterMode::Stream => IndexEntry::Stream(self.stream()),
            IndexIterMode::Block | IndexIterMode::NonEmptyBlock => IndexEntry::Block(self.block()),
            IndexIterMode::Any => {
                // For Any mode, we need to determine what we're currently pointing at
                // This is a simplification - we'll return Block by default
                IndexEntry::Block(self.block())
            }
        }
    }

    /// Returns a mutable reference to the underlying raw `lzma_index_iter` struct.
    #[inline]
    pub(crate) fn as_mut_raw(&mut self) -> &mut liblzma_sys::lzma_index_iter {
        &mut self.inner
    }
}

impl Iterator for IndexIterator<'_> {
    type Item = IndexEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if ffi::lzma_index_iter_next(self, self.mode) {
            Some(self.current_entry())
        } else {
            None
        }
    }
}

/// Iteration mode for [`IndexIterator`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexIterMode {
    /// Iterate over any entry (streams and blocks).
    Any,
    /// Iterate over streams only.
    Stream,
    /// Iterate over blocks only.
    Block,
    /// Iterate over blocks that contain uncompressed data.
    NonEmptyBlock,
}

impl From<IndexIterMode> for liblzma_sys::lzma_index_iter_mode {
    fn from(mode: IndexIterMode) -> Self {
        match mode {
            IndexIterMode::Any => liblzma_sys::lzma_index_iter_mode_LZMA_INDEX_ITER_ANY,
            IndexIterMode::Stream => liblzma_sys::lzma_index_iter_mode_LZMA_INDEX_ITER_STREAM,
            IndexIterMode::Block => liblzma_sys::lzma_index_iter_mode_LZMA_INDEX_ITER_BLOCK,
            IndexIterMode::NonEmptyBlock => {
                liblzma_sys::lzma_index_iter_mode_LZMA_INDEX_ITER_NONEMPTY_BLOCK
            }
        }
    }
}

/// Stream flags containing metadata about the stream format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFlags {
    /// Stream format version (currently always 0).
    pub version: u32,
    /// Size of the Index field (in the Stream Footer).
    /// Set to `None` when read from Stream Header.
    pub backward_size: Option<u64>,
    /// Integrity check algorithm used for this stream.
    pub check: IntegrityCheck,
}

impl StreamFlags {
    /// Convert from a raw liblzma stream flags pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be valid and point to a properly initialized `lzma_stream_flags`.
    pub(crate) unsafe fn from_raw(ptr: *const liblzma_sys::lzma_stream_flags) -> Option<Self> {
        // backward_size is set to LZMA_VLI_UNKNOWN when not available (e.g., from Stream Header)
        const LZMA_VLI_UNKNOWN: u64 = u64::MAX;

        if ptr.is_null() {
            return None;
        }

        let raw = &*ptr;

        // Try to convert the check type
        let check = IntegrityCheck::try_from(raw.check).ok()?;

        let backward_size = if raw.backward_size == LZMA_VLI_UNKNOWN {
            None
        } else {
            Some(raw.backward_size)
        };

        Some(Self {
            version: raw.version,
            backward_size,
            check,
        })
    }

    /// Convert this value into a raw `liblzma_sys::lzma_stream_flags`.
    ///
    /// When `backward_size` is `None`, the returned struct will have `backward_size`
    /// set to `LZMA_VLI_UNKNOWN` (represented as `u64::MAX`).
    pub(crate) fn to_raw(self) -> liblzma_sys::lzma_stream_flags {
        // backward_size is set to LZMA_VLI_UNKNOWN when not available (e.g., from Stream Header).
        const LZMA_VLI_UNKNOWN: u64 = u64::MAX;

        // SAFETY: `lzma_stream_flags` is a plain C struct; all-zero initialization is valid.
        let mut raw: liblzma_sys::lzma_stream_flags = unsafe { std::mem::zeroed() };
        raw.version = self.version;
        raw.backward_size = self.backward_size.unwrap_or(LZMA_VLI_UNKNOWN);
        raw.check = self.check.into();
        raw
    }

    /// Decode an XZ Stream Header.
    pub fn decode_header(input: &[u8; crate::stream::HEADER_SIZE]) -> Result<Self> {
        ffi::decode_stream_header_flags(input)
    }

    /// Decode an XZ Stream Footer.
    pub fn decode_footer(input: &[u8; crate::stream::HEADER_SIZE]) -> Result<Self> {
        ffi::decode_stream_footer_flags(input)
    }

    /// Decode and compare Stream Header and Stream Footer flags.
    pub fn compare_header_footer(
        header: &[u8; crate::stream::HEADER_SIZE],
        footer: &[u8; crate::stream::HEADER_SIZE],
    ) -> Result<()> {
        ffi::compare_stream_header_footer(header, footer)
    }
}

/// Information about a stream stored within an index.
#[derive(Debug, Clone)]
pub struct StreamInfo {
    /// Stream number (1-based).
    pub number: u64,
    /// Number of blocks in the stream.
    pub block_count: u64,
    /// Compressed start offset.
    pub compressed_offset: u64,
    /// Uncompressed start offset.
    pub uncompressed_offset: u64,
    /// Compressed size (without padding).
    pub compressed_size: u64,
    /// Uncompressed size.
    pub uncompressed_size: u64,
    /// Padding size following the stream.
    pub padding: u64,
    /// Stream flags containing format metadata.
    pub flags: Option<StreamFlags>,
}

/// Information about a block stored within an index.
#[derive(Debug, Clone)]
pub struct BlockInfo {
    /// Block number within the current stream (1-based).
    pub number_in_stream: u64,
    /// Block number within the entire file (1-based).
    pub number_in_file: u64,
    /// Compressed start offset within the file.
    pub compressed_file_offset: u64,
    /// Uncompressed start offset within the file.
    pub uncompressed_file_offset: u64,
    /// Total compressed size (including headers).
    pub total_size: u64,
    /// Uncompressed size.
    pub uncompressed_size: u64,
    /// Unpadded size.
    pub unpadded_size: u64,
}

#[cfg(test)]
mod tests {
    use crate::{Action, Stream};

    use super::*;

    /// Helper function to create a `FileInfoDecoder` with extracted index.
    ///
    /// This compresses the provided `data`, then decodes the resulting file info and index.
    ///
    /// # Parameters
    ///
    /// * `data` - The input data to compress and decode.
    ///
    /// # Returns
    ///
    /// Returns `Some(FileInfoDecoder)` with a finished and valid decoder if successful,
    /// or `None` on error.
    fn create_test_decoder(data: &[u8]) -> Option<crate::FileInfoDecoder> {
        use crate::encoder::options::{Compression, IntegrityCheck};

        // Compress input data.
        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level6, IntegrityCheck::Crc64)
            .ok()?;

        // Allocate sufficient buffer for compression output.
        let mut compressed = vec![0u8; data.len().saturating_mul(2) + 2048];
        let (_, written) = encoder.process(data, &mut compressed, Action::Run).ok()?;
        let mut total_written = written;

        // Ensure all data is compressed by finishing the stream.
        let (_, finish_written) = encoder
            .process(&[], &mut compressed[total_written..], Action::Finish)
            .ok()?;
        total_written += finish_written;
        compressed.truncate(total_written);

        // Create a decoder to extract the index and file info.
        let mut decoder = Stream::default()
            .file_info_decoder(u64::MAX, compressed.len() as u64)
            .ok()?;

        let mut pos = 0;
        let mut pending_action = Action::Run;
        // Drive the decoder with all input, respecting seek requests.
        while !decoder.is_finished() {
            match decoder.process(&compressed[pos..], pending_action) {
                Ok(bytes_read) => {
                    pos += bytes_read;
                    // If input is exhausted, switch to Finish action to complete.
                    if pos >= compressed.len() && pending_action != Action::Finish {
                        pending_action = Action::Finish;
                    }
                }
                Err(crate::Error::SeekNeeded) => {
                    // Handle random-access request by updating position.
                    #[allow(clippy::cast_possible_truncation)]
                    {
                        pos = decoder.seek_pos() as usize;
                    }
                    pending_action = Action::Run; // Always resume with Run after seek.
                }
                Err(crate::Error::StreamEnd) => break,
                Err(_) => return None,
            }

            // If we've finished processing the buffer and issued Finish, we're done.
            if pos >= compressed.len() && pending_action == Action::Finish {
                break;
            }
        }

        // Decoder should be finished, and index extractable.
        Some(decoder)
    }

    /// Test basic Index creation and accessors.
    #[test]
    fn index_basic_accessors() {
        let test_data = b"Lazzy dog jumps over the lazy fox".repeat(100);
        let decoder = create_test_decoder(&test_data).unwrap();
        let index = decoder.index().unwrap();

        // Basic checks
        assert!(index.stream_count() > 0);
        assert!(index.block_count() > 0);
        assert!(index.file_size() > 0);
        assert!(index.uncompressed_size() > 0);
        assert_eq!(index.uncompressed_size(), test_data.len() as u64);

        // File size should be smaller than uncompressed for compressible data
        assert!(index.file_size() < index.uncompressed_size());
    }

    /// Test `Index::checks()` returns non-zero value.
    #[test]
    fn index_checks_non_zero() {
        let test_data = b"Lazzy dog jumps over the lazy fox";
        let decoder = create_test_decoder(test_data).unwrap();
        let index = decoder.index().unwrap();

        let checks = index.checks();
        assert_ne!(checks, 0, "Checks should not be zero");
    }

    /// Test `IndexIterator` with Stream mode using Iterator trait.
    #[test]
    fn index_iterator_streams_trait() {
        let test_data = b"Lazzy dog jumps over the lazy fox".repeat(50);
        let decoder = create_test_decoder(&test_data).unwrap();
        let index = decoder.index().unwrap();

        let streams: Vec<_> = index.iter_streams().collect();
        assert_eq!(streams.len() as u64, index.stream_count());

        for entry in streams {
            if let IndexEntry::Stream(stream_info) = entry {
                assert!(stream_info.block_count > 0);
                assert!(stream_info.compressed_size > 0);
                assert!(stream_info.uncompressed_size > 0);
            } else {
                panic!("Expected Stream entry");
            }
        }
    }

    /// Test `IndexIterator` with Block mode using Iterator trait.
    #[test]
    fn index_iterator_blocks_trait() {
        let test_data = b"Lazzy dog jumps over the lazy fox".repeat(50);
        let decoder = create_test_decoder(&test_data).unwrap();
        let index = decoder.index().unwrap();

        let blocks: Vec<_> = index.iter_blocks().collect();
        assert_eq!(blocks.len() as u64, index.block_count());

        for (i, entry) in blocks.iter().enumerate() {
            if let IndexEntry::Block(block_info) = entry {
                assert_eq!(block_info.number_in_file, (i + 1) as u64);
                assert!(block_info.total_size > 0);
                assert!(block_info.uncompressed_size > 0);
            } else {
                panic!("Expected Block entry");
            }
        }
    }

    /// Test `IndexIterator` with `NonEmptyBlock` mode using Iterator trait.
    #[test]
    fn index_iterator_non_empty_blocks_trait() {
        let test_data = b"Lazzy dog jumps over the lazy fox".repeat(50);
        let decoder = create_test_decoder(&test_data).unwrap();
        let index = decoder.index().unwrap();

        let blocks: Vec<_> = index.iter_non_empty_blocks().collect();
        assert!(!blocks.is_empty());

        for entry in blocks {
            if let IndexEntry::Block(block_info) = entry {
                assert!(block_info.uncompressed_size > 0);
            } else {
                panic!("Expected Block entry");
            }
        }
    }

    /// Test `IndexIterator` with Any mode using Iterator trait.
    #[test]
    fn index_iterator_any_trait() {
        let test_data = b"Lazzy dog jumps over the lazy fox".repeat(50);
        let decoder = create_test_decoder(&test_data).unwrap();
        let index = decoder.index().unwrap();

        let entries: Vec<_> = index.iter().collect();
        assert!(!entries.is_empty(), "Should have at least one entry");
    }

    /// Test that iterator returns None when no more entries (Iterator trait).
    #[test]
    fn index_iterator_returns_none_at_end() {
        let test_data = b"Lazzy dog jumps over the lazy fox";
        let decoder = create_test_decoder(test_data).unwrap();
        let index = decoder.index().unwrap();

        let mut iter = index.iter_blocks();

        // Exhaust the iterator
        while iter.next().is_some() {}

        // Next call should return None
        assert!(iter.next().is_none());
    }

    /// Test `StreamInfo` fields are populated correctly using Iterator trait.
    #[test]
    fn index_iterator_stream_info_fields() {
        let test_data = b"Lazzy dog jumps over the lazy fox".repeat(100);
        let decoder = create_test_decoder(&test_data).unwrap();
        let index = decoder.index().unwrap();

        // Should have at least one stream
        let entry = index.iter_streams().next().unwrap();

        if let IndexEntry::Stream(stream_info) = entry {
            // Check all fields are reasonable
            assert_eq!(stream_info.number, 1);
            assert!(stream_info.block_count > 0);
            assert_eq!(stream_info.compressed_offset, 0);
            assert_eq!(stream_info.uncompressed_offset, 0);
            assert!(stream_info.compressed_size > 0);
            assert!(stream_info.uncompressed_size > 0);
        } else {
            panic!("Expected Stream entry");
        }
    }

    /// Test `BlockInfo` fields are populated correctly using Iterator trait.
    #[test]
    fn index_iterator_block_info_fields() {
        let test_data = b"Lazzy dog jumps over the lazy fox".repeat(100);
        let decoder = create_test_decoder(&test_data).unwrap();
        let index = decoder.index().unwrap();

        // Should have at least one block
        let entry = index.iter_blocks().next().unwrap();

        if let IndexEntry::Block(block_info) = entry {
            // Check all fields are reasonable
            assert_eq!(block_info.number_in_stream, 1);
            assert_eq!(block_info.number_in_file, 1);
            // Offsets should be >= 0
            assert!(block_info.compressed_file_offset > 0);
            assert_eq!(block_info.uncompressed_file_offset, 0);
            assert!(block_info.total_size > 0);
            assert!(block_info.uncompressed_size > 0);
            assert!(block_info.unpadded_size > 0);
            // Unpadded size should be <= total size
            assert!(block_info.unpadded_size <= block_info.total_size);
        } else {
            panic!("Expected Block entry");
        }
    }

    /// Test creating multiple iterators using Iterator trait.
    #[test]
    fn index_multiple_iterators_trait() {
        let test_data = b"Lazzy dog jumps over the lazy fox";
        let decoder = create_test_decoder(test_data).unwrap();
        let index = decoder.index().unwrap();

        // Create first iterator
        let count1 = index.iter_blocks().count();

        // Create second iterator - should iterate independently
        let count2 = index.iter_blocks().count();

        assert_eq!(count1, count2);
        assert_eq!(count1 as u64, index.block_count());
    }

    /// Test that `StreamFlags` are properly extracted and parsed.
    #[test]
    fn index_stream_flags() {
        let test_data = b"Lazzy dog jumps over the lazy fox".repeat(10);
        let decoder = create_test_decoder(&test_data).unwrap();
        let index = decoder.index().unwrap();

        // Get first stream
        let entry = index.iter_streams().next().unwrap();
        if let IndexEntry::Stream(stream_info) = entry {
            // StreamFlags should be present
            let flags = stream_info.flags.expect("StreamFlags should be present");

            // Check version (should be 0 for current XZ format)
            assert_eq!(flags.version, 0);

            // Check that integrity check is valid
            assert!(matches!(
                flags.check,
                IntegrityCheck::None
                    | IntegrityCheck::Crc32
                    | IntegrityCheck::Crc64
                    | IntegrityCheck::Sha256
            ));

            // backward_size should be present (we're reading from Stream Footer)
            assert!(flags.backward_size.is_some());
        } else {
            panic!("Expected Stream entry");
        }
    }

    /// Test `IndexIterMode` conversion.
    #[test]
    fn index_iter_mode_conversion() {
        use liblzma_sys::*;

        let any: lzma_index_iter_mode = IndexIterMode::Any.into();
        assert_eq!(any, lzma_index_iter_mode_LZMA_INDEX_ITER_ANY);

        let stream: lzma_index_iter_mode = IndexIterMode::Stream.into();
        assert_eq!(stream, lzma_index_iter_mode_LZMA_INDEX_ITER_STREAM);

        let block: lzma_index_iter_mode = IndexIterMode::Block.into();
        assert_eq!(block, lzma_index_iter_mode_LZMA_INDEX_ITER_BLOCK);

        let non_empty: lzma_index_iter_mode = IndexIterMode::NonEmptyBlock.into();
        assert_eq!(
            non_empty,
            lzma_index_iter_mode_LZMA_INDEX_ITER_NONEMPTY_BLOCK
        );
    }
}
