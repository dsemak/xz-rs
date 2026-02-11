//! Thin wrappers around the `liblzma` FFI calls used by the safe API.

use std::ptr;

use crate::error::{result_from_lzma_ret, Result};
use crate::stream::StreamFlags;
use crate::{decoder, encoder, Action, Error, Index, IndexIterMode, IndexIterator, Stream};

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

/// Populate `lzma_options_lzma` from an `xz(1)`-compatible preset via `lzma_lzma_preset`.
pub(crate) fn lzma_lzma_preset(
    options: &mut liblzma_sys::lzma_options_lzma,
    preset: u32,
) -> Result<()> {
    // SAFETY: `options` is a valid pointer; liblzma writes the output in place.
    let failed = unsafe { liblzma_sys::lzma_lzma_preset(options, preset) };
    if failed != 0 {
        return Err(Error::OptionsError);
    }
    Ok(())
}

/// Initialise a legacy `.lzma` encoder via `lzma_alone_encoder`.
pub(crate) fn lzma_alone_encoder(
    options: &liblzma_sys::lzma_options_lzma,
    stream: &mut Stream,
) -> Result<()> {
    // SAFETY:
    // - The stream is valid and not already initialized.
    // - `options` is a valid pointer to an initialized options struct.
    let ret = unsafe { liblzma_sys::lzma_alone_encoder(stream.lzma_stream(), options) };
    result_from_lzma_ret(ret, ())
}

/// Initialise an index decoder with `lzma_index_decoder`.
///
/// The index will be made available through the `index_ptr` after decoding completes.
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
/// The combined index will be made available through the `index_ptr` after decoding completes.
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

/// Decode XZ Stream Header flags into a Rust structure.
pub(crate) fn decode_stream_header_flags(
    input: &[u8; crate::stream::HEADER_SIZE],
) -> Result<StreamFlags> {
    // SAFETY: `lzma_stream_flags` is a plain C struct; all-zero initialization is valid.
    let mut raw: liblzma_sys::lzma_stream_flags = unsafe { std::mem::zeroed() };
    // SAFETY: `raw` is a valid out-pointer and `input` points to
    // `LZMA_STREAM_HEADER_SIZE` bytes.
    let ret =
        unsafe { liblzma_sys::lzma_stream_header_decode(ptr::from_mut(&mut raw), input.as_ptr()) };
    result_from_lzma_ret(ret, ())?;

    // SAFETY: `raw` is initialized by liblzma.
    unsafe { StreamFlags::from_raw(ptr::from_ref(&raw)) }.ok_or(Error::OptionsError)
}

/// Decode XZ Stream Footer flags.
pub(crate) fn decode_stream_footer_flags(
    input: &[u8; crate::stream::HEADER_SIZE],
) -> Result<StreamFlags> {
    // SAFETY: `lzma_stream_flags` is a plain C struct; all-zero initialization is valid.
    let mut flags: liblzma_sys::lzma_stream_flags = unsafe { std::mem::zeroed() };
    // SAFETY:
    // - `flags` is a valid out-pointer for liblzma to write to.
    // - `input.as_ptr()` points to exactly `LZMA_STREAM_HEADER_SIZE` bytes.
    let ret = unsafe {
        liblzma_sys::lzma_stream_footer_decode(ptr::from_mut(&mut flags), input.as_ptr())
    };
    result_from_lzma_ret(ret, ())?;

    // SAFETY: `flags` is initialized by liblzma.
    unsafe { StreamFlags::from_raw(ptr::from_ref(&flags)) }.ok_or(Error::OptionsError)
}

/// Decode and compare Stream Header and Stream Footer flags.
pub(crate) fn compare_stream_header_footer(
    header: &[u8; crate::stream::HEADER_SIZE],
    footer: &[u8; crate::stream::HEADER_SIZE],
) -> Result<()> {
    // SAFETY: `lzma_stream_flags` is a plain C struct; all-zero initialization is valid.
    let mut hdr: liblzma_sys::lzma_stream_flags = unsafe { std::mem::zeroed() };
    let mut ftr: liblzma_sys::lzma_stream_flags = unsafe { std::mem::zeroed() };
    // SAFETY:
    // - `hdr` is a valid out-pointer for liblzma to write to.
    // - `header.as_ptr()` points to exactly `LZMA_STREAM_HEADER_SIZE` bytes.
    let ret =
        unsafe { liblzma_sys::lzma_stream_header_decode(ptr::from_mut(&mut hdr), header.as_ptr()) };
    result_from_lzma_ret(ret, ())?;

    // SAFETY:
    // - `ftr` is a valid out-pointer for liblzma to write to.
    // - `footer.as_ptr()` points to exactly `LZMA_STREAM_HEADER_SIZE` bytes.
    let ret =
        unsafe { liblzma_sys::lzma_stream_footer_decode(ptr::from_mut(&mut ftr), footer.as_ptr()) };
    result_from_lzma_ret(ret, ())?;

    // SAFETY: Both pointers are valid for the duration of the call.
    let ret =
        unsafe { liblzma_sys::lzma_stream_flags_compare(ptr::from_ref(&hdr), ptr::from_ref(&ftr)) };
    result_from_lzma_ret(ret, ())
}

/// Decode an XZ Index field from a buffer.
///
/// Returns the decoded [`Index`] and the number of bytes consumed.
pub(crate) fn decode_xz_index_field(
    memlimit: &mut u64,
    input: &[u8],
    allocator: Option<&crate::stream::LzmaAllocator>,
) -> Result<(Index, usize)> {
    let allocator_ptr = allocator.map_or(std::ptr::null(), crate::stream::LzmaAllocator::as_ptr);
    let mut index_ptr: *mut liblzma_sys::lzma_index = ptr::null_mut();
    let mut in_pos: usize = 0;

    // SAFETY:
    // - `index_ptr` is a valid out-pointer for liblzma to write to.
    // - `memlimit` is a valid in/out pointer; liblzma may update it on
    //   `LZMA_MEMLIMIT_ERROR`.
    // - `allocator_ptr` is either NULL (use malloc/free) or points to a valid
    //   liblzma allocator vtable for the duration of the call.
    // - `input.as_ptr()` points to `input.len()` readable bytes.
    // - `in_pos` is a valid out-pointer; liblzma updates it on success.
    let ret = unsafe {
        liblzma_sys::lzma_index_buffer_decode(
            ptr::from_mut(&mut index_ptr),
            ptr::from_mut(memlimit),
            allocator_ptr,
            input.as_ptr(),
            ptr::from_mut(&mut in_pos),
            input.len(),
        )
    };
    result_from_lzma_ret(ret, ())?;

    // SAFETY: `index_ptr` is returned by liblzma on success and ownership is transferred to `Index`.
    let index = unsafe { Index::from_raw(index_ptr, None).ok_or(Error::MemError)? };
    Ok((index, in_pos))
}

/// Set Stream Flags for the last Stream in an index.
pub(crate) fn lzma_index_stream_flags(index: &mut Index, flags: &StreamFlags) -> Result<()> {
    let raw = flags.to_raw();
    // SAFETY:
    // - `index.as_mut_ptr()` is a valid pointer to an `lzma_index` owned by `Index`.
    // - `raw` is a properly initialized `lzma_stream_flags` value.
    let ret = unsafe { liblzma_sys::lzma_index_stream_flags(index.as_mut_ptr(), &raw const raw) };
    result_from_lzma_ret(ret, ())
}

/// Set Stream Padding for the last Stream in an index.
pub(crate) fn lzma_index_stream_padding(index: &mut Index, padding: u64) -> Result<()> {
    // SAFETY: `index` is valid. `padding` is validated by liblzma.
    let ret = unsafe { liblzma_sys::lzma_index_stream_padding(index.as_mut_ptr(), padding) };
    result_from_lzma_ret(ret, ())
}

/// Return the total size of a Stream represented by the index.
pub(crate) fn lzma_index_stream_size(index: &Index) -> u64 {
    // SAFETY: The index pointer is valid and owned by the caller.
    unsafe { liblzma_sys::lzma_index_stream_size(index.as_ptr()) }
}

/// Concatenate two indexes.
pub(crate) fn lzma_index_cat(
    dest: &mut Index,
    src: &mut Index,
    allocator: Option<&crate::stream::LzmaAllocator>,
) -> Result<()> {
    let allocator_ptr = allocator.map_or(std::ptr::null(), crate::stream::LzmaAllocator::as_ptr);
    // SAFETY:
    // - `dest.as_mut_ptr()` and `src.as_mut_ptr()` are valid pointers to `lzma_index`
    //   instances for the duration of the call.
    // - `allocator_ptr` is either NULL (use malloc/free) or points to a valid allocator
    //   vtable. It must be compatible with how `src` was allocated.
    //
    // Note: On success, liblzma frees/moves the contents of `src` into `dest`, and any
    // iterators to `src` become invalid.
    let ret =
        unsafe { liblzma_sys::lzma_index_cat(dest.as_mut_ptr(), src.as_mut_ptr(), allocator_ptr) };
    result_from_lzma_ret(ret, ())
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

/// Returns `true` if the linked liblzma supports the given check ID.
pub(crate) fn lzma_check_is_supported(check_id: u32) -> bool {
    // SAFETY: `lzma_check_is_supported` is a pure function that doesn't keep the
    // pointer; it only inspects the passed check ID.
    unsafe { liblzma_sys::lzma_check_is_supported(check_id) != 0 }
}
