//! Gunzip decompression utility  
//!
//! A modern Rust implementation of the gunzip decompression utility.

use std::io;
use gzip_utils::{run_cli, CliConfig, OperationMode};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = "gunzip";

    let config = CliConfig {
        mode: OperationMode::Decompress,
        keep: false,
        force: false,
        verbose: false,
        stdout: false,
        level: None,
    };

    run_cli(&args[1..], &config, program)
}
