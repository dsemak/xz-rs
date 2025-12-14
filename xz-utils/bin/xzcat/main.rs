//! XZcat utility - decompress and output to stdout
//!
//! A pure-Rust implementation of xzcat for viewing compressed files.

use std::io;
use xz_utils::{run_cli, CliConfig, CompressionFormat, OperationMode};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = "xzcat";
    
    let config = CliConfig {
        mode: OperationMode::Cat,
        format: CompressionFormat::Auto,  // Auto-detect format
        stdout: true,                     // Always output to stdout
        keep: true,                       // Never remove input files
        force: false,
        verbose: false,
        level: None,
    };
    
    run_cli(&args[1..], &config, program)
}
