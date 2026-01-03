//! Safe, high-level bindings to the liblzma encoder/decoder used by XZ Utils.
//!
//! The crate wraps the `liblzma` C API in resource-owning Rust types. The
//! [`Stream`] type tracks the raw `lzma_stream`, while [`Encoder`] and
//! [`Decoder`] expose streaming compression and decompression with idiomatic
//! error handling.
//!
//! # Highlights
//!
//! - safe wrappers around the raw FFI surface
//! - automatic resource cleanup through RAII
//! - support for XZ and legacy LZMA formats, including multi-threaded mode
//! - optional custom allocators
//!
//! # Example
//!
//! ```rust
//! use lzma_safe::{Action, Stream};
//! use lzma_safe::encoder::options::{Compression, IntegrityCheck};
//!
//! let stream = Stream::default();
//! let mut encoder = stream.easy_encoder(Compression::Level6, IntegrityCheck::Crc64)?;
//! let input = b"hello from liblzma";
//! let mut compressed = vec![0_u8; 128];
//! let (_, len) = encoder.process(input, &mut compressed, Action::Finish)?;
//! compressed.truncate(len);
//!
//! let stream = Stream::default();
//! let mut decoder = stream.auto_decoder(u64::MAX, Default::default())?;
//! let mut output = vec![0_u8; 64];
//! let (_, len) = decoder.process(&compressed, &mut output, Action::Finish)?;
//! assert_eq!(&output[..len], input);
//! # Ok::<(), lzma_safe::Error>(())
//! ```

pub mod decoder;
pub mod encoder;
pub mod stream;

mod error;
mod ffi;

pub use decoder::{Decoder, FileInfoDecoder, IndexDecoder};
pub use encoder::{AloneEncoder, Encoder};
pub use error::{Error, Result};
pub use stream::{BlockInfo, Index, IndexEntry, IndexIterMode, IndexIterator, Stream, StreamInfo};

/// Access to the liblzma version reported by the linked C library.
pub struct Version;

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // SAFETY: liblzma guarantees that the returned pointer is valid and null-terminated.
        let c_str = unsafe { std::ffi::CStr::from_ptr(liblzma_sys::lzma_version_string()) };
        write!(f, "{}", c_str.to_string_lossy())
    }
}

impl Version {
    /// Returns the packed `MMmmmmpp` version number produced by liblzma.
    pub fn number() -> u32 {
        // SAFETY: liblzma_version_number() is always safe to call and returns a valid u32.
        unsafe { liblzma_sys::lzma_version_number() }
    }
}

/// High-level equivalent of `lzma_action` used by [`Encoder`] and [`Decoder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Continue processing input data (`LZMA_RUN`).
    Run,

    /// Finalise the stream; no more input will be provided (`LZMA_FINISH`).
    Finish,

    /// Flush pending data while keeping the stream open (`LZMA_SYNC_FLUSH`).
    SyncFlush,

    /// Finish the current block and create a recovery point (`LZMA_FULL_FLUSH`).
    FullFlush,

    /// Finish the current block and insert an MT decoding barrier (`LZMA_FULL_BARRIER`).
    FullBarrier,
}

impl From<Action> for liblzma_sys::lzma_action {
    fn from(action: Action) -> Self {
        match action {
            Action::Run => liblzma_sys::lzma_action_LZMA_RUN,
            Action::Finish => liblzma_sys::lzma_action_LZMA_FINISH,
            Action::SyncFlush => liblzma_sys::lzma_action_LZMA_SYNC_FLUSH,
            Action::FullFlush => liblzma_sys::lzma_action_LZMA_FULL_FLUSH,
            Action::FullBarrier => liblzma_sys::lzma_action_LZMA_FULL_BARRIER,
        }
    }
}
