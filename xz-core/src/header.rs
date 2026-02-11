//! Helpers for reading and inspecting XZ stream headers.

use std::io;

/// Size of the XZ Stream Header in bytes (12 bytes).
pub const XZ_STREAM_HEADER_SIZE: usize = lzma_safe::stream::HEADER_SIZE;

/// Magic bytes at the beginning of an XZ Stream Header.
pub const XZ_STREAM_HEADER_MAGIC: [u8; 6] = lzma_safe::stream::HEADER_MAGIC;

/// Reads up to one XZ Stream Header from `input` without requiring EOF.
///
/// This is intended for callers that want to peek at the header (e.g. to inspect the
/// integrity check type) and then re-chain the bytes back into the decode stream.
///
/// # Errors
///
/// Returns an error if reading from `input` fails.
pub fn read_xz_stream_header_prefix(input: &mut impl io::Read) -> io::Result<Vec<u8>> {
    let mut prefix = Vec::with_capacity(XZ_STREAM_HEADER_SIZE);
    let mut tmp = [0_u8; XZ_STREAM_HEADER_SIZE];

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
