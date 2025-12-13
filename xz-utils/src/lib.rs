//! Common CLI utilities and shared functionality for XZ/LZMA tools.

mod config;
mod error;
mod io;
mod operations;
mod process;

#[cfg(test)]
mod tests;

pub use config::{CliConfig, CompressionFormat, OperationMode, XZ_EXTENSION, LZMA_EXTENSION, DEFAULT_BUFFER_SIZE};
pub use error::{Error, Result};
pub use io::{generate_output_filename, has_compression_extension, open_input, open_output};
pub use operations::{compress_file, decompress_file};
pub use process::{cleanup_input_file, process_file, run_cli};
