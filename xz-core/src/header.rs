//! Helpers for reading and inspecting XZ stream headers.

use std::io;

/// Size of the legacy LZMA_Alone header in bytes.
pub const LZMA_ALONE_HEADER_SIZE: usize = lzma_safe::LZMA_ALONE_HEADER_SIZE;

/// Magic bytes at the beginning of an XZ Stream Header.
pub const XZ_STREAM_HEADER_MAGIC: [u8; 6] = lzma_safe::stream::HEADER_MAGIC;

/// Magic bytes at the beginning of an lzip member.
pub const LZIP_HEADER_MAGIC: [u8; 4] = *b"LZIP";

/// Number of bytes needed to distinguish the auto-detected decoder formats.
pub const DECODE_FORMAT_PROBE_SIZE: usize = LZMA_ALONE_HEADER_SIZE;

/// Reads enough bytes from `input` to identify the formats supported by auto-detection.
///
/// The returned prefix can be chained back into the decode stream after probing.
///
/// # Errors
///
/// Returns an error if reading from `input` fails.
pub fn read_decode_format_probe_prefix(input: &mut impl io::Read) -> io::Result<Vec<u8>> {
    let mut prefix = Vec::with_capacity(DECODE_FORMAT_PROBE_SIZE);
    let mut tmp = [0_u8; DECODE_FORMAT_PROBE_SIZE];

    while prefix.len() < tmp.len() {
        let offset = prefix.len();
        let n = input.read(&mut tmp[offset..])?;
        if n == 0 {
            break;
        }
        prefix.extend_from_slice(&tmp[offset..offset + n]);
    }

    Ok(prefix)
}

/// Detects an unsupported XZ integrity check ID from a Stream Header prefix.
///
/// Returns `Some(check_id)` when the input begins with a valid XZ Stream Header magic
/// and the check type is not supported by the linked liblzma.
pub fn detect_unsupported_xz_check_id(prefix: &[u8]) -> Option<u32> {
    if prefix.starts_with(&XZ_STREAM_HEADER_MAGIC)
        && prefix.len() >= lzma_safe::stream::BLOCK_HEADER_SIZE_MIN
    {
        let check_id = u32::from(prefix[lzma_safe::stream::BLOCK_HEADER_SIZE_MIN - 1]);
        (!lzma_safe::lzma_check_is_supported(check_id)).then_some(check_id)
    } else {
        None
    }
}

/// Returns `true` when the probe prefix looks like `.xz`, legacy `.lzma`, or `.lz`.
pub fn is_known_decode_format(prefix: &[u8]) -> bool {
    prefix.starts_with(&XZ_STREAM_HEADER_MAGIC)
        || prefix.starts_with(&LZIP_HEADER_MAGIC)
        || is_lzma_alone_header(prefix)
}

/// Returns `true` when the probe prefix looks like a legacy `.lzma` header.
fn is_lzma_alone_header(prefix: &[u8]) -> bool {
    if prefix.len() < LZMA_ALONE_HEADER_SIZE {
        return false;
    }

    let properties = prefix[0];
    if properties >= 9 * 5 * 5 {
        return false;
    }

    let mut dict_size_bytes = [0_u8; 4];
    dict_size_bytes.copy_from_slice(&prefix[1..5]);
    let dict_size = u32::from_le_bytes(dict_size_bytes);
    if dict_size != u32::MAX && !is_picky_lzma_dict_size(dict_size) {
        return false;
    }

    let mut uncompressed_size_bytes = [0_u8; 8];
    uncompressed_size_bytes.copy_from_slice(&prefix[5..LZMA_ALONE_HEADER_SIZE]);
    let uncompressed_size = u64::from_le_bytes(uncompressed_size_bytes);
    uncompressed_size == u64::MAX || uncompressed_size < (1_u64 << 38)
}

/// Returns `true` when the LZMA dictionary size is valid.
fn is_picky_lzma_dict_size(dict_size: u32) -> bool {
    if dict_size == 0 {
        return false;
    }

    let mut rounded = dict_size - 1;
    rounded |= rounded >> 2;
    rounded |= rounded >> 3;
    rounded |= rounded >> 4;
    rounded |= rounded >> 8;
    rounded |= rounded >> 16;
    rounded = rounded.wrapping_add(1);

    rounded == dict_size
}

#[cfg(test)]
mod tests {
    use super::{
        is_known_decode_format, read_decode_format_probe_prefix, LZIP_HEADER_MAGIC,
        LZMA_ALONE_HEADER_SIZE, XZ_STREAM_HEADER_MAGIC,
    };

    /// Detect `.xz` input from the stream header magic.
    #[test]
    fn detects_xz_probe_prefix() {
        let mut prefix = Vec::from(XZ_STREAM_HEADER_MAGIC);
        prefix.resize(LZMA_ALONE_HEADER_SIZE, 0);
        assert!(is_known_decode_format(&prefix));
    }

    /// Detect lzip input from the member magic.
    #[test]
    fn detects_lzip_probe_prefix() {
        let mut prefix = Vec::from(LZIP_HEADER_MAGIC);
        prefix.resize(LZMA_ALONE_HEADER_SIZE, 0);
        assert!(is_known_decode_format(&prefix));
    }

    /// Detect a plausible legacy `.lzma` header.
    #[test]
    fn detects_lzma_alone_probe_prefix() {
        #[rustfmt::skip]
        let prefix = [
            0x5D,                                           // lc/lp/pb
            0x00, 0x00, 0x80, 0x00,                         // 8 MiB dictionary
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // unknown size
        ];
        assert!(is_known_decode_format(&prefix));
    }

    /// Reject arbitrary input that doesn't match any supported format.
    #[test]
    fn rejects_unknown_probe_prefix() {
        assert!(!is_known_decode_format(b"foo"));
    }

    /// Read at most the auto-detect probe size from the input.
    #[test]
    fn reads_decode_probe_prefix_without_requiring_eof() {
        #[rustfmt::skip]
        let input = [
            0x5D,                                           // lc/lp/pb
            0x00, 0x00, 0x80, 0x00,                         // 8 MiB dictionary
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // unknown size
            b'x',                                           // trailing bytes
            b'y',
        ];
        let mut cursor = std::io::Cursor::new(input);

        let prefix = read_decode_format_probe_prefix(&mut cursor).unwrap();

        assert_eq!(prefix.len(), LZMA_ALONE_HEADER_SIZE);
    }
}
