//! High-level API for extracting metadata from XZ files.

use std::io::{Read, Seek, SeekFrom};
use std::num::NonZeroU64;

use lzma_safe::{
    Action, BlockInfo as LzmaBlockInfo, FileInfoDecoder, IndexEntry, Stream,
    StreamInfo as LzmaStreamInfo,
};

use crate::{Error, Result};

/// Detailed information about an XZ file including streams and blocks.
pub struct FileInfo {
    /// Decoder that owns the index
    decoder: FileInfoDecoder,
    /// Total file size
    file_size: u64,
}

impl FileInfo {
    /// Get the number of streams in the file.
    pub fn stream_count(&self) -> u64 {
        self.decoder.index().map_or(0, |index| index.stream_count())
    }

    /// Get the total number of blocks in all streams.
    pub fn block_count(&self) -> u64 {
        self.decoder.index().map_or(0, |index| index.block_count())
    }

    /// Get the compressed file size.
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Get the total uncompressed size.
    pub fn uncompressed_size(&self) -> u64 {
        self.decoder
            .index()
            .map_or(0, |index| index.uncompressed_size())
    }

    /// Get the bitmask of integrity checks used.
    pub fn checks(&self) -> u32 {
        self.decoder.index().map_or(0, |index| index.checks())
    }

    /// Collect all streams into a vector.
    ///
    /// # Returns
    ///
    /// Returns a vector of [`StreamInfo`] objects.
    pub fn streams(&self) -> Vec<StreamInfo> {
        self.decoder
            .index()
            .map(|index| {
                index
                    .iter_streams()
                    .filter_map(|entry| {
                        if let IndexEntry::Stream(info) = entry {
                            Some(StreamInfo::from_lzma_stream_info(info))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns a vector containing metadata for all blocks within the XZ file.
    pub fn blocks(&self) -> Vec<BlockInfo> {
        self.decoder
            .index()
            .map(|index| {
                index
                    .iter_blocks()
                    .filter_map(|entry| {
                        if let IndexEntry::Block(info) = entry {
                            Some(BlockInfo::from_lzma_block_info(info))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
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
    // Get file size by seeking to the end
    let file_size = reader.seek(SeekFrom::End(0))?;

    // Check if file is empty
    if file_size == 0 {
        return Err(Error::InvalidOption("File is empty".into()));
    }

    // Check minimum file size (12 bytes for stream header + 12 bytes for stream footer)
    const MIN_XZ_SIZE: u64 = 2 * lzma_safe::stream::HEADER_SIZE as u64;
    if file_size < MIN_XZ_SIZE {
        return Err(Error::InvalidOption(
            "File is too small to be a valid XZ file".into(),
        ));
    }

    // Seek back to the beginning
    reader.seek(SeekFrom::Start(0))?;

    // Create decoder with memory limit
    let memlimit_value = memlimit.map_or(u64::MAX, |v| v.get());
    let mut decoder = Stream::default().file_info_decoder(memlimit_value, file_size)?;

    // Read and process the file in chunks
    const CHUNK_SIZE: usize = 64 * 1024; // 64 KB chunks
    let mut buffer = vec![0u8; CHUNK_SIZE];
    // Number of bytes currently pending (available at the start of `buffer`)
    let mut pending_len: usize = 0;
    // Action to use for the next decoder call
    let mut action = Action::Run;

    loop {
        // Ensure we have some input to feed the decoder, or we're finishing.
        if pending_len == 0 && action != Action::Finish {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                // No more data available; switch to Finish to let decoder complete.
                action = Action::Finish;
            } else {
                pending_len = read;
            }
        }

        // Feed the pending input to the decoder.
        match decoder.process(&buffer[..pending_len], action) {
            Ok(consumed) => {
                if decoder.is_finished() {
                    break;
                }

                if consumed == 0 {
                    // Decoder made no progress with current input.
                    if action == Action::Finish {
                        // We're already finishing but decoder didn't complete.
                        return Err(Error::InvalidOption(
                            "Decoder did not finish after processing all available data".into(),
                        ));
                    }

                    // Read more data and append.
                    // Grow the buffer if it's full.
                    if pending_len == buffer.len() {
                        let grow_by = CHUNK_SIZE;
                        // Try to reserve to avoid potential OOM aborts on resize
                        buffer
                            .try_reserve(grow_by)
                            .map_err(|_| Error::AllocationFailed {
                                capacity: buffer.len() + grow_by,
                            })?;
                        let old_len = buffer.len();
                        buffer.resize(old_len + grow_by, 0);
                    }

                    // Read after the pending bytes to extend the available window.
                    let read = reader.read(&mut buffer[pending_len..])?;
                    if read == 0 {
                        // No more bytes available; switch to Finish to let decoder complete.
                        action = Action::Finish;
                    } else {
                        pending_len += read;
                    }
                    // Continue the loop to process with the extended input.
                } else {
                    // Move the unconsumed tail to the beginning of the buffer.
                    let remaining = pending_len - consumed;
                    if remaining > 0 {
                        buffer.copy_within(consumed..pending_len, 0);
                    }
                    pending_len = remaining;
                    // On next iteration we will either read more (if remaining == 0)
                    // or continue processing the remaining bytes.
                }
            }
            Err(lzma_safe::Error::SeekNeeded) => {
                // Decoder needs to seek to a specific position.
                // liblzma won't ask to seek past the known size of the input file.
                let seek_pos = decoder.seek_pos();
                reader.seek(SeekFrom::Start(seek_pos))?;

                // In C example they set strm->avail_in = 0 after LZMA_SEEK_NEEDED.
                // This clears the internal state. Try calling process() with empty
                // input to achieve the same effect.
                let _ = decoder.process(&[], Action::Run);

                // The old data in the buffer is useless now. Set pending_len to zero
                // so that we will read new input from the new file position on the
                // next iteration of this loop (even if seek_pos equals the current position).
                pending_len = 0;
                // Always resume with Run after seek.
                action = Action::Run;
            }
            Err(e) => {
                return Err(Error::Backend(e));
            }
        }
    }

    Ok(FileInfo { decoder, file_size })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

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
