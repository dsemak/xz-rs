//! XZ compression utility
//!
//! A pure-Rust implementation of the xz compression utility.

use std::io;
use xz_utils::{run_cli, CliConfig, CompressionFormat, OperationMode};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = "xz";
    
    // Default configuration for compression
    let config = CliConfig {
        mode: OperationMode::Compress,
        format: CompressionFormat::Xz,  // Default to XZ format
        level: Some(6),                 // Default compression level
        keep: false,                    // Remove original files
        force: false,                   // Don't overwrite existing files
        verbose: false,                 // Quiet mode
        stdout: false,                  // Output to file
    };
    
    run_cli(&args[1..], &config, program)
}
