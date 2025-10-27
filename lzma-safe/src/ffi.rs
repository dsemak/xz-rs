//! Thin wrappers around the `liblzma` FFI calls used by the safe API.

use crate::error::{result_from_lzma_ret, Result};
use crate::{decoder, encoder, Action, Index, IndexIterMode, IndexIterator, Stream};

/// Call `lzma_code` with a safe return type.
pub(crate) fn lzma_code(stream: &mut Stream, action: Action) -> Result<()> {
    // SAFETY: The stream is assumed to be valid and initialized by liblzma.
    let ret = unsafe { liblzma_sys::lzma_code(stream.lzma_stream(), action.into()) };
    result_from_lzma_ret(ret, ())
}

/// Finalise a stream by calling `lzma_end`.
pub(crate) fn lzma_end(mut stream: Stream) {
    // SAFETY: The stream is valid and initialized by liblzma.
    // This function can only be called once per stream due to move semantics.
    unsafe { liblzma_sys::lzma_end(stream.lzma_stream()) };
}

/// Initialise a single-threaded encoder via `lzma_easy_encoder`.
pub(crate) fn lzma_easy_encoder(
    level: encoder::options::Compression,
    check: encoder::options::IntegrityCheck,
    stream: &mut Stream,
) -> Result<()> {
    // SAFETY: The stream must be valid and not already initialized.
    // The level and check must be valid for liblzma.
    let ret = unsafe {
        liblzma_sys::lzma_easy_encoder(stream.lzma_stream(), level.to_preset(), check.into())
    };
    result_from_lzma_ret(ret, ())
}

/// Initialise a multithreaded encoder via `lzma_stream_encoder_mt`.
pub(crate) fn lzma_stream_encoder_mt(
    config: &encoder::Options,
    stream: &mut Stream,
) -> Result<Option<encoder::options::RawFilters>> {
    // Build mt options and prepare filter chain in this scope so pointers stay valid.
    let (mt, raw_filters) = config.to_lzma_options();

    // SAFETY: `mt` and its filter chain memory are valid for the duration of this call.
    let ret = unsafe { liblzma_sys::lzma_stream_encoder_mt(stream.lzma_stream(), &raw const mt) };
    result_from_lzma_ret(ret, raw_filters)
}

/// Initialise an XZ decoder with `lzma_stream_decoder`.
pub(crate) fn lzma_stream_decoder(
    memlimit: u64,
    flags: decoder::options::Flags,
    stream: &mut Stream,
) -> Result<()> {
    // SAFETY: The stream is valid and not already initialized.
    // The flags are validated by the type system.
    let ret = unsafe {
        liblzma_sys::lzma_stream_decoder(stream.lzma_stream(), memlimit, flags.to_liblzma_flags())
    };
    result_from_lzma_ret(ret, ())
}

/// Initialise a multithreaded decoder with `lzma_stream_decoder_mt`.
pub(crate) fn lzma_stream_decoder_mt(
    options: &decoder::Options,
    stream: &mut Stream,
) -> Result<()> {
    // SAFETY: All fields of the options struct are set as required by liblzma documentation.
    // The stream is valid and not already initialized.
    let ret = unsafe {
        liblzma_sys::lzma_stream_decoder_mt(stream.lzma_stream(), &options.to_lzma_options())
    };
    result_from_lzma_ret(ret, ())
}

/// Initialise an auto-detecting decoder via `lzma_auto_decoder`.
pub(crate) fn lzma_auto_decoder(
    memlimit: u64,
    flags: decoder::options::Flags,
    stream: &mut Stream,
) -> Result<()> {
    // SAFETY: The stream is valid and not already initialized.
    // The flags are validated by the type system.
    let ret = unsafe {
        liblzma_sys::lzma_auto_decoder(stream.lzma_stream(), memlimit, flags.to_liblzma_flags())
    };
    result_from_lzma_ret(ret, ())
}

/// Initialise a legacy LZMA decoder via `lzma_alone_decoder`.
pub(crate) fn lzma_alone_decoder(memlimit: u64, stream: &mut Stream) -> Result<()> {
    // SAFETY: The stream is valid and not already initialized.
    let ret = unsafe { liblzma_sys::lzma_alone_decoder(stream.lzma_stream(), memlimit) };
    result_from_lzma_ret(ret, ())
}

/// Initialise an index decoder with `lzma_index_decoder`.
///
/// The index will be made available through the index_ptr after decoding completes.
pub(crate) fn lzma_index_decoder(
    stream: &mut Stream,
    index_ptr: *mut *mut liblzma_sys::lzma_index,
    memlimit: u64,
) -> Result<()> {
    // SAFETY: The stream is valid and not already initialized.
    // The index_ptr will be populated when decoding completes successfully.
    let ret = unsafe { liblzma_sys::lzma_index_decoder(stream.lzma_stream(), index_ptr, memlimit) };
    result_from_lzma_ret(ret, ())
}

/// Initialise a file info decoder with `lzma_file_info_decoder`.
///
/// The combined index will be made available through the index_ptr after decoding completes.
pub(crate) fn lzma_file_info_decoder(
    stream: &mut Stream,
    index_ptr: *mut *mut liblzma_sys::lzma_index,
    memlimit: u64,
    file_size: u64,
) -> Result<()> {
    // SAFETY: The stream is valid and not already initialized.
    // The index_ptr will be populated when decoding completes successfully.
    let ret = unsafe {
        liblzma_sys::lzma_file_info_decoder(stream.lzma_stream(), index_ptr, memlimit, file_size)
    };
    result_from_lzma_ret(ret, ())
}

/// Initializes an `lzma_index_iter` for traversing an index.
pub(crate) fn lzma_index_iter_init(iter: &mut liblzma_sys::lzma_index_iter, index: &Index) {
    // SAFETY: Both `iter` and `index` are valid and properly initialized.
    // The `iter` memory must be zeroed as required by liblzma.
    unsafe {
        liblzma_sys::lzma_index_iter_init(iter, index.as_ptr());
    }
}

/// Free an `lzma_index` previously allocated by liblzma.
pub(crate) fn lzma_index_end(
    index: *mut liblzma_sys::lzma_index,
    allocator: Option<&crate::stream::LzmaAllocator>,
) {
    let allocator_ptr = allocator.map_or(std::ptr::null(), crate::stream::LzmaAllocator::as_ptr);
    unsafe { liblzma_sys::lzma_index_end(index, allocator_ptr) };
}

/// Advance the given `lzma_index_iter` to the next entry using the provided mode.
///
/// # Returns
///
/// `true` if the iterator points to a valid entry after advancing, or `false` if the end is reached.
pub(crate) fn lzma_index_iter_next(iter: &mut IndexIterator, mode: IndexIterMode) -> bool {
    // SAFETY: `iter` points to a valid iterator and `mode` is a trusted enum.
    unsafe {
        // liblzma returns zero (0) for "success" (i.e., valid entry found) and nonzero for "end".
        liblzma_sys::lzma_index_iter_next(iter.as_mut_raw(), mode.into()) == 0
    }
}

/// Returns the number of streams present in the given `Index`.
pub(crate) fn lzma_index_stream_count(index: &Index) -> u64 {
    // SAFETY: The index pointer is valid and owned by the caller.
    unsafe { liblzma_sys::lzma_index_stream_count(index.as_ptr()) }
}

/// Returns the number of blocks present in the given `Index`.
pub(crate) fn lzma_index_block_count(index: &Index) -> u64 {
    // SAFETY: The index pointer is valid and owned by the caller.
    unsafe { liblzma_sys::lzma_index_block_count(index.as_ptr()) }
}

/// Returns the total compressed file size tracked by the given `Index`.
pub(crate) fn lzma_index_file_size(index: &Index) -> u64 {
    // SAFETY: The index pointer is valid and owned by the caller.
    unsafe { liblzma_sys::lzma_index_file_size(index.as_ptr()) }
}

/// Returns the total uncompressed size tracked by the given `Index`.
pub(crate) fn lzma_index_uncompressed_size(index: &Index) -> u64 {
    // SAFETY: The index pointer is valid and owned by the caller.
    unsafe { liblzma_sys::lzma_index_uncompressed_size(index.as_ptr()) }
}

/// Returns a bitmask of integrity checks found in the given `Index`.
pub(crate) fn lzma_index_checks(index: &Index) -> u32 {
    // SAFETY: The index pointer is valid and owned by the caller.
    unsafe { liblzma_sys::lzma_index_checks(index.as_ptr()) }
}

/// Estimate decoder memory usage for a given compression preset.
#[allow(dead_code)]
pub(crate) fn lzma_easy_decoder_memusage(level: encoder::options::Compression) -> u64 {
    // SAFETY: The compression level is validated by the type system and converted safely.
    unsafe { liblzma_sys::lzma_easy_decoder_memusage(level.to_preset()) }
}

/// Update the runtime memory limit of a decoder stream.
#[allow(dead_code)]
pub(crate) fn lzma_memlimit_set(memlimit: u64, stream: &mut Stream) -> Result<()> {
    // SAFETY: The stream is assumed to be valid and initialized by liblzma.
    let ret = unsafe { liblzma_sys::lzma_memlimit_set(stream.lzma_stream(), memlimit) };
    result_from_lzma_ret(ret, ())
}
