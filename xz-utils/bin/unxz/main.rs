//! XZ decompression utility
//!
//! A pure-Rust implementation of the unxz decompression utility.

use std::io;
use xz_utils::{run_cli, CliConfig, CompressionFormat, OperationMode};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = "unxz";
    
    let config = CliConfig {
        mode: OperationMode::Decompress,
        format: CompressionFormat::Auto,  // Auto-detect format
        keep: false,                      // Remove .xz/.lzma files
        force: false,
        verbose: false,
        stdout: false,
        level: None,                      // Not used for decompression
    };
    
    run_cli(&args[1..], &config, program)
}
