//! High-level API for extracting metadata from XZ files.

use std::io::{Read, Seek, SeekFrom};
use std::num::NonZeroU64;

use lzma_safe::stream::StreamFlags;
use lzma_safe::{BlockInfo as LzmaBlockInfo, Index, IndexEntry, StreamInfo as LzmaStreamInfo};

use crate::{Error, Result};

/// Size of an XZ Stream Header/Footer in bytes.
const STREAM_HEADER_SIZE: usize = lzma_safe::stream::HEADER_SIZE;
const STREAM_HEADER_SIZE_U64: u64 = STREAM_HEADER_SIZE as u64;

/// Stream Padding is a sequence of `0x00` bytes whose size is a multiple of four bytes.
const STREAM_PADDING_ALIGNMENT_BYTES: u64 = 4;
const STREAM_PADDING_WORD_SIZE: usize = STREAM_PADDING_ALIGNMENT_BYTES as usize;

/// Minimum size of a valid XZ stream: header + footer.
const MIN_STREAM_SIZE: u64 = 2 * STREAM_HEADER_SIZE_U64;

/// Detailed information about an XZ file including streams and blocks.
pub struct FileInfo {
    /// Decoded Index for the whole file (may contain multiple Streams).
    index: Index,
    /// Total file size
    file_size: u64,
}

impl FileInfo {
    /// Get the number of streams in the file.
    pub fn stream_count(&self) -> u64 {
        self.index.stream_count()
    }

    /// Get the total number of blocks in all streams.
    pub fn block_count(&self) -> u64 {
        self.index.block_count()
    }

    /// Get the compressed file size.
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Get the total uncompressed size.
    pub fn uncompressed_size(&self) -> u64 {
        self.index.uncompressed_size()
    }

    /// Get the bitmask of integrity checks used.
    pub fn checks(&self) -> u32 {
        self.index.checks()
    }

    /// Collect all streams into a vector.
    ///
    /// # Returns
    ///
    /// Returns a vector of [`StreamInfo`] objects.
    pub fn streams(&self) -> Vec<StreamInfo> {
        self.index
            .iter_streams()
            .filter_map(|entry| {
                if let IndexEntry::Stream(info) = entry {
                    Some(StreamInfo::from_lzma_stream_info(info))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns a vector containing metadata for all blocks within the XZ file.
    pub fn blocks(&self) -> Vec<BlockInfo> {
        self.index
            .iter_blocks()
            .filter_map(|entry| {
                if let IndexEntry::Block(info) = entry {
                    Some(BlockInfo::from_lzma_block_info(info))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Information about a stream within an XZ file.
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
}

impl StreamInfo {
    fn from_lzma_stream_info(info: LzmaStreamInfo) -> Self {
        Self {
            number: info.number,
            block_count: info.block_count,
            compressed_offset: info.compressed_offset,
            uncompressed_offset: info.uncompressed_offset,
            compressed_size: info.compressed_size,
            uncompressed_size: info.uncompressed_size,
            padding: info.padding,
        }
    }
}

/// Information about a block within an XZ file.
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

impl BlockInfo {
    fn from_lzma_block_info(info: LzmaBlockInfo) -> Self {
        Self {
            number_in_stream: info.number_in_stream,
            number_in_file: info.number_in_file,
            compressed_file_offset: info.compressed_file_offset,
            uncompressed_file_offset: info.uncompressed_file_offset,
            total_size: info.total_size,
            uncompressed_size: info.uncompressed_size,
            unpadded_size: info.unpadded_size,
        }
    }
}

/// Read exactly `buf.len()` bytes at an absolute file offset.
fn read_exact_at<R: Read + Seek>(reader: &mut R, offset: u64, buf: &mut [u8]) -> Result<()> {
    reader.seek(SeekFrom::Start(offset))?;
    reader.read_exact(buf)?;
    Ok(())
}

/// Read an XZ Stream Header (`LZMA_STREAM_HEADER_SIZE`) at an absolute file offset.
fn read_stream_header_at<R: Read + Seek>(
    reader: &mut R,
    offset: u64,
) -> Result<[u8; STREAM_HEADER_SIZE]> {
    let mut header = [0u8; STREAM_HEADER_SIZE];
    read_exact_at(reader, offset, &mut header)?;
    Ok(header)
}

/// Read an XZ Stream Footer (`LZMA_STREAM_HEADER_SIZE`) at an absolute file offset.
fn read_stream_footer_at<R: Read + Seek>(
    reader: &mut R,
    offset: u64,
) -> Result<[u8; STREAM_HEADER_SIZE]> {
    let mut footer = [0u8; STREAM_HEADER_SIZE];
    read_exact_at(reader, offset, &mut footer)?;
    Ok(footer)
}

/// Returns `true` if the given padding word is all zero bytes.
fn is_zero_padding_word(word: &[u8; STREAM_PADDING_WORD_SIZE]) -> bool {
    word.iter().all(|b| *b == 0)
}

/// Consume Stream Padding bytes preceding `pos`.
///
/// Returns `(new_pos, padding_len)` where `new_pos` points to the end of the Stream Footer.
fn consume_stream_padding<R: Read + Seek>(reader: &mut R, mut pos: u64) -> Result<(u64, u64)> {
    let mut padding: u64 = 0;

    while pos >= STREAM_PADDING_ALIGNMENT_BYTES {
        let mut word = [0u8; STREAM_PADDING_WORD_SIZE];
        read_exact_at(reader, pos - STREAM_PADDING_ALIGNMENT_BYTES, &mut word)?;
        if is_zero_padding_word(&word) {
            padding += STREAM_PADDING_ALIGNMENT_BYTES;
            pos -= STREAM_PADDING_ALIGNMENT_BYTES;
        } else {
            break;
        }
    }

    Ok((pos, padding))
}

/// Validate the Index size from Stream Footer and convert it to `usize`.
///
/// The XZ format stores Index size (Backward Size) in bytes and it must be
/// a multiple of four bytes.
fn checked_index_len(index_size: u64) -> Result<usize> {
    if !index_size.is_multiple_of(STREAM_PADDING_ALIGNMENT_BYTES) {
        return Err(Error::InvalidOption(
            "Index size in Stream Footer is not a multiple of 4".into(),
        ));
    }

    usize::try_from(index_size)
        .map_err(|_| Error::InvalidOption("Index size is too large for this platform".into()))
}

/// Parse a single XZ Stream by reading the Stream Footer, Index field, and Stream Header.
///
/// Returns the decoded [`Index`] and the Stream start offset.
fn parse_stream_from_end<R: Read + Seek>(
    reader: &mut R,
    footer_end: u64,
    memlimit: u64,
) -> Result<(Index, u64)> {
    if footer_end < MIN_STREAM_SIZE {
        return Err(Error::InvalidOption(
            "File is too small to contain a complete XZ stream".into(),
        ));
    }

    let footer_start = footer_end - STREAM_HEADER_SIZE_U64;
    let footer = read_stream_footer_at(reader, footer_start)?;

    let footer_flags = StreamFlags::decode_footer(&footer).map_err(Error::Backend)?;
    let Some(index_size_u64) = footer_flags.backward_size else {
        return Err(Error::InvalidOption(
            "Stream Footer did not contain Backward Size".into(),
        ));
    };

    let index_len = checked_index_len(index_size_u64)?;

    let index_end = footer_start;
    if index_end < index_size_u64 {
        return Err(Error::InvalidOption(
            "Stream Footer Backward Size points outside of the file".into(),
        ));
    }
    let index_start = index_end - index_size_u64;

    let mut index_buf = vec![0u8; index_len];
    read_exact_at(reader, index_start, &mut index_buf)?;

    let mut index = Index::decode_xz_index_field(&index_buf, memlimit).map_err(Error::Backend)?;
    index
        .set_stream_flags_from_footer(&footer)
        .map_err(Error::Backend)?;

    let stream_size = index.stream_size();
    if stream_size < MIN_STREAM_SIZE {
        return Err(Error::InvalidOption(
            "Decoded stream size is too small".into(),
        ));
    }
    if stream_size > footer_end {
        return Err(Error::InvalidOption(
            "Decoded stream size points outside of the file".into(),
        ));
    }

    let stream_start = footer_end - stream_size;
    let header = read_stream_header_at(reader, stream_start)?;
    StreamFlags::compare_header_footer(&header, &footer).map_err(Error::Backend)?;

    Ok((index, stream_start))
}

/// Extracts file information from an XZ file.
///
/// This function reads the XZ file index without decompressing the actual data,
/// providing metadata about streams, blocks, sizes, and compression ratios.
///
/// # Parameters
///
/// * `reader` - A readable and seekable input containing the XZ file
/// * `memlimit` - Optional memory limit for the decoder (defaults to `u64::MAX`)
///
/// # Returns
///
/// Returns [`FileInfo`] containing detailed metadata about the XZ file.
///
/// # Errors
///
/// Returns an error if:
///
/// - The file is not a valid XZ file
/// - Seeking fails
/// - The file is corrupted
/// - Memory limit is exceeded
pub fn extract_file_info<R: Read + Seek>(
    reader: &mut R,
    memlimit: Option<NonZeroU64>,
) -> Result<FileInfo> {
    let file_size = reader.seek(SeekFrom::End(0))?;
    if file_size == 0 {
        return Err(Error::InvalidOption("File is empty".into()));
    }

    if file_size < MIN_STREAM_SIZE {
        return Err(Error::InvalidOption(
            "File is too small to be a valid XZ file".into(),
        ));
    }

    let memlimit_value = memlimit.map_or(u64::MAX, |v| v.get());

    // Parse concatenated Streams from the end of the file.
    let mut pos = file_size;
    let mut indices_rev: Vec<Index> = Vec::new();

    while pos > 0 {
        // Stream Padding consists of 0x00 bytes and its size is a multiple of four bytes.
        let (footer_end, padding) = consume_stream_padding(reader, pos)?;

        let (mut index, stream_start) = parse_stream_from_end(reader, footer_end, memlimit_value)?;
        index.set_stream_padding(padding).map_err(Error::Backend)?;

        indices_rev.push(index);
        pos = stream_start;

        // If we've reached the beginning, stop.
        if pos == 0 {
            break;
        }
    }

    if indices_rev.is_empty() {
        return Err(Error::InvalidOption(
            "No XZ streams were found in the input".into(),
        ));
    }

    indices_rev.reverse();
    let mut it = indices_rev.into_iter();
    let mut combined = it
        .next()
        .ok_or_else(|| Error::InvalidOption("No XZ streams were found in the input".into()))?;
    for idx in it {
        combined.append(idx).map_err(Error::Backend)?;
    }

    Ok(FileInfo {
        index: combined,
        file_size,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use lzma_safe::{Action, Stream};

    use crate::ratio;

    use super::*;

    /// Test basic file info extraction.
    #[test]
    fn test_extract_file_info_basic() {
        use lzma_safe::encoder::options::{Compression, IntegrityCheck};

        // Create test data
        let test_data = b"The quick brown fox jumps over the lazy dog".repeat(10);

        // Compress it
        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level6, IntegrityCheck::Crc64)
            .unwrap();

        let mut compressed = vec![0u8; test_data.len() * 2];
        let (_, written) = encoder
            .process(&test_data, &mut compressed, Action::Run)
            .unwrap();
        let mut total = written;
        let (_, finish) = encoder
            .process(&[], &mut compressed[total..], Action::Finish)
            .unwrap();
        total += finish;
        compressed.truncate(total);

        // Extract file info
        let mut cursor = Cursor::new(compressed);
        let info = extract_file_info(&mut cursor, None).unwrap();

        assert!(info.stream_count() > 0);
        assert!(info.block_count() > 0);
        assert!(info.file_size() > 0);
        assert_eq!(info.uncompressed_size(), test_data.len() as u64);
    }

    /// Test extraction with large data to verify buffer handling.
    #[test]
    fn test_extract_file_info_large_data() {
        use lzma_safe::encoder::options::{Compression, IntegrityCheck};

        // Create large test data (10 MB)
        let test_data = vec![b'x'; 10 * 1024 * 1024];

        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level3, IntegrityCheck::Sha256)
            .unwrap();

        let mut compressed = Vec::new();
        let mut temp_buffer = vec![0u8; 64 * 1024];

        let mut offset = 0;
        while offset < test_data.len() {
            let chunk_size = (test_data.len() - offset).min(8192);
            let (consumed, written) = encoder
                .process(
                    &test_data[offset..offset + chunk_size],
                    &mut temp_buffer,
                    Action::Run,
                )
                .unwrap();
            compressed.extend_from_slice(&temp_buffer[..written]);
            offset += consumed;
        }

        loop {
            let (_, written) = encoder
                .process(&[], &mut temp_buffer, Action::Finish)
                .unwrap();
            compressed.extend_from_slice(&temp_buffer[..written]);
            if encoder.is_finished() {
                break;
            }
        }

        let mut cursor = Cursor::new(&compressed);
        let info = extract_file_info(&mut cursor, None).unwrap();

        assert_eq!(info.uncompressed_size(), test_data.len() as u64);
        assert!(info.file_size() < test_data.len() as u64);
        assert!(ratio(info.file_size(), info.uncompressed_size()) < 100.0);
        assert_eq!(info.stream_count(), 1);
    }

    /// Test extraction with empty data.
    #[test]
    fn test_extract_file_info_empty() {
        use lzma_safe::encoder::options::{Compression, IntegrityCheck};

        let test_data = b"";

        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level6, IntegrityCheck::Crc32)
            .unwrap();

        let mut compressed = vec![0u8; 1024];
        let (_, written) = encoder
            .process(test_data, &mut compressed, Action::Finish)
            .unwrap();
        compressed.truncate(written);

        let mut cursor = Cursor::new(compressed);
        let info = extract_file_info(&mut cursor, None).unwrap();

        assert_eq!(info.uncompressed_size(), 0);
        assert!(info.file_size() > 0);
    }

    /// Test stream and block metadata extraction.
    #[test]
    fn test_extract_file_info_streams_and_blocks() {
        use lzma_safe::encoder::options::{Compression, IntegrityCheck};

        let test_data = b"Hello, World!".repeat(100);

        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level6, IntegrityCheck::Crc64)
            .unwrap();

        let mut compressed = vec![0u8; test_data.len() * 2];
        let (_, written) = encoder
            .process(&test_data, &mut compressed, Action::Run)
            .unwrap();
        let mut total = written;
        let (_, finish) = encoder
            .process(&[], &mut compressed[total..], Action::Finish)
            .unwrap();
        total += finish;
        compressed.truncate(total);

        let mut cursor = Cursor::new(compressed);
        let info = extract_file_info(&mut cursor, None).unwrap();

        // Check streams
        let streams = info.streams();
        assert!(!streams.is_empty());
        for stream in &streams {
            assert!(stream.number > 0);
            assert!(stream.uncompressed_size > 0);
            assert!(stream.compressed_size > 0);
        }

        // Check blocks
        let blocks = info.blocks();
        assert!(!blocks.is_empty());
        for block in &blocks {
            assert!(block.number_in_file > 0);
            assert!(block.uncompressed_size > 0);
            assert!(block.total_size > 0);
        }
    }

    /// Test compression ratio calculation.
    #[test]
    fn test_extract_file_info_compression_ratio() {
        use lzma_safe::encoder::options::{Compression, IntegrityCheck};

        // Highly compressible data
        let test_data = b"aaaaaaaaaa".repeat(1000);

        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level9, IntegrityCheck::Crc64)
            .unwrap();

        let mut compressed = vec![0u8; test_data.len() * 2];
        let (_, written) = encoder
            .process(&test_data, &mut compressed, Action::Run)
            .unwrap();
        let mut total = written;
        let (_, finish) = encoder
            .process(&[], &mut compressed[total..], Action::Finish)
            .unwrap();
        total += finish;
        compressed.truncate(total);

        let mut cursor = Cursor::new(compressed);
        let info = extract_file_info(&mut cursor, None).unwrap();

        // Highly compressible data should have good ratio
        assert!(ratio(info.file_size(), info.uncompressed_size()) < 10.0);
        assert_eq!(info.uncompressed_size(), test_data.len() as u64);
    }

    /// Test with different compression levels.
    #[test]
    fn test_extract_file_info_different_levels() {
        use lzma_safe::encoder::options::{Compression, IntegrityCheck};

        let test_data = b"The quick brown fox jumps over the lazy dog".repeat(20);

        for level in [
            Compression::Level0,
            Compression::Level3,
            Compression::Level6,
            Compression::Level9,
        ] {
            let mut encoder = Stream::default()
                .easy_encoder(level, IntegrityCheck::Crc64)
                .unwrap();

            let mut compressed = vec![0u8; test_data.len() * 2];
            let (_, written) = encoder
                .process(&test_data, &mut compressed, Action::Run)
                .unwrap();
            let mut total = written;
            let (_, finish) = encoder
                .process(&[], &mut compressed[total..], Action::Finish)
                .unwrap();
            total += finish;
            compressed.truncate(total);

            let mut cursor = Cursor::new(compressed);
            let info = extract_file_info(&mut cursor, None).unwrap();

            assert_eq!(info.uncompressed_size(), test_data.len() as u64);
            assert!(info.file_size() > 0);
            assert!(info.stream_count() > 0);
        }
    }

    /// Test with different integrity checks.
    #[test]
    fn test_extract_file_info_integrity_checks() {
        use lzma_safe::encoder::options::{Compression, IntegrityCheck};

        let test_data = b"Test data for integrity checks".repeat(10);

        for check in [
            IntegrityCheck::Crc32,
            IntegrityCheck::Crc64,
            IntegrityCheck::Sha256,
        ] {
            let mut encoder = Stream::default()
                .easy_encoder(Compression::Level6, check)
                .unwrap();

            let mut compressed = vec![0u8; test_data.len() * 2];
            let (_, written) = encoder
                .process(&test_data, &mut compressed, Action::Run)
                .unwrap();
            let mut total = written;
            let (_, finish) = encoder
                .process(&[], &mut compressed[total..], Action::Finish)
                .unwrap();
            total += finish;
            compressed.truncate(total);

            let mut cursor = Cursor::new(compressed);
            let info = extract_file_info(&mut cursor, None).unwrap();

            assert_eq!(info.uncompressed_size(), test_data.len() as u64);
            assert!(info.checks() != 0);
        }
    }

    /// Test block and stream ratio calculations.
    #[test]
    fn test_block_and_stream_ratios() {
        use lzma_safe::encoder::options::{Compression, IntegrityCheck};

        let test_data = b"Sample data ".repeat(50);

        let mut encoder = Stream::default()
            .easy_encoder(Compression::Level6, IntegrityCheck::Crc64)
            .unwrap();

        let mut compressed = vec![0u8; test_data.len() * 2];
        let (_, written) = encoder
            .process(&test_data, &mut compressed, Action::Run)
            .unwrap();
        let mut total = written;
        let (_, finish) = encoder
            .process(&[], &mut compressed[total..], Action::Finish)
            .unwrap();
        total += finish;
        compressed.truncate(total);

        let mut cursor = Cursor::new(compressed);
        let info = extract_file_info(&mut cursor, None).unwrap();

        // Check stream ratios
        for stream in info.streams() {
            let ratio = ratio(stream.compressed_size, stream.uncompressed_size);
            assert!(ratio > 0.0);
            assert!(ratio < 100.0);
        }

        // Check block ratios
        for block in info.blocks() {
            let ratio = ratio(block.total_size, block.uncompressed_size);
            assert!(ratio > 0.0);
        }
    }
}
