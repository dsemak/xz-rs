//! Common CLI utilities and shared functionality for XZ command-line tools.
//!
//! This module provides high-level abstractions for XZ compression and decompression
//! operations, file I/O handling, and CLI configuration management. It serves as the
//! primary interface between command-line tools and the core XZ functionality.

mod config;
mod error;
mod io;
mod operations;
mod process;
mod utils;

#[cfg(test)]
mod tests;

pub use config::{CliConfig, OperationMode, DEFAULT_BUFFER_SIZE, LZMA_EXTENSION, XZ_EXTENSION};
pub use error::{format_error_for_stderr, CliError, Error, InvocationError, Result, Warning};
pub use io::{generate_output_filename, has_compression_extension, open_input, open_output};
pub use operations::{compress_file, decompress_file};
pub use process::{cleanup_input_file, parse_memory_limit, process_file, run_cli};
pub use utils::argfiles;
