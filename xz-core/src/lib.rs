//! # xz-core
//!
//! High-performance, memory-safe streaming compression and decompression for XZ format.
//!
//! This crate provides ergonomic, production-ready abstractions for working with XZ (LZMA2)
//! compressed data streams. It features both synchronous and asynchronous APIs with built-in
//! security protections, automatic resource management, and comprehensive error handling.
//!
//! ## Key Features
//!
//! - **Memory Safety**: Built on safe Rust with automatic bounds checking and resource cleanup
//! - **Performance**: Multi-threaded compression/decompression with configurable thread pools
//! - **Security**: Built-in protections against decompression bombs and memory exhaustion attacks
//! - **Flexibility**: Support for both XZ and legacy LZMA formats with format auto-detection
//! - **Async Support**: First-class async/await support with efficient streaming
//! - **Resource Control**: Configurable memory limits, timeouts, and buffer sizes
//!
//! ## Quick Start
//!
//! ### Synchronous Compression
//!
//! ```rust
//! use std::io::Cursor;
//!
//! use xz_core::{
//!     options::CompressionOptions,
//!     pipeline::compress,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let input = b"The quick brown fox jumps over the lazy dog";
//! let mut output = Vec::new();
//!
//! let options = CompressionOptions::default();
//! let summary = compress(
//!     &mut Cursor::new(input),
//!     &mut output,
//!     &options,
//! )?;
//!
//! println!("Compressed {} bytes to {} bytes",
//!          summary.bytes_read, summary.bytes_written);
//! # Ok(())
//! # }
//! ```
//!
//! ### Synchronous Decompression
//!
//! ```rust
//! use std::io::Cursor;
//!
//! use xz_core::{
//!     options::DecompressionOptions,
//!     pipeline::decompress,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let compressed_data = vec![0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]; // Mock XZ data
//! let mut output = Vec::new();
//!
//! let options = DecompressionOptions::default();
//! let summary = decompress(
//!     &mut Cursor::new(&compressed_data),
//!     &mut output,
//!     &options,
//! )?;
//!
//! println!("Decompressed {} bytes to {} bytes",
//!          summary.bytes_read, summary.bytes_written);
//! # Ok(())
//! # }
//! ```
//!
//! ### Asynchronous Processing
//!
//! ```rust
//! use std::io::Cursor;
//!
//! use xz_core::{
//!     options::CompressionOptions,
//!     pipeline::compress_async,
//! };
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let input = b"The quick brown fox jumps over the lazy dog";
//! let mut output = Vec::new();
//!
//! let options = CompressionOptions::default();
//! let summary = compress_async(
//!     &mut Cursor::new(input),
//!     &mut output,
//!     &options,
//! ).await?;
//!
//! println!("Async compressed {} bytes", summary.bytes_read);
//! # Ok(())
//! # }
//! ```
//!
//! ## Configuration
//!
//! ### Compression Settings
//!
//! ```rust
//! use std::num::NonZeroU64;
//!
//! use lzma_safe::encoder::options::{Compression, IntegrityCheck};
//!
//! use xz_core::{
//!     options::CompressionOptions,
//!     Threading,
//! };
//!
//! let options = CompressionOptions::default()
//!     .with_level(Compression::Level9)           // Maximum compression
//!     .with_threads(Threading::Exact(8))         // Use 8 threads
//!     .with_check(IntegrityCheck::Sha256)        // Strong integrity check
//!     .with_block_size(Some(NonZeroU64::new(16 * 1024 * 1024).unwrap())); // 16MB blocks
//! ```
//!
//! ### Secure Decompression
//!
//! ```rust
//! use xz_core::{
//!     options::DecompressionOptions,
//!     config::DecodeMode,
//! };
//! use std::num::NonZeroU64;
//!
//! let options = DecompressionOptions::default()
//!     .with_memlimit(NonZeroU64::new(64 * 1024 * 1024).unwrap())     // 64MB soft limit
//!     .with_memlimit_stop(Some(NonZeroU64::new(128 * 1024 * 1024).unwrap())) // 128MB hard limit
//!     .with_mode(DecodeMode::Auto);                                   // Auto-detect format
//! ```

mod buffer;
mod error;
mod threading;

pub mod config;
pub mod options;
pub mod pipeline;

pub use crate::error::{BackendError, Error, Result};
pub use crate::threading::Threading;
pub use buffer::{Allocator, Buffer, Deallocator, DeallocatorFn, GlobalAllocator};
