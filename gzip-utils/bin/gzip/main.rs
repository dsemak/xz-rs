//! Gzip compression utility
//!
//! A modern Rust implementation of the gzip compression utility.

use std::io;
use gzip_utils::{run_cli, CliConfig, OperationMode};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = "gzip";

    let config = CliConfig {
        mode: OperationMode::Compress,
        level: Some(6),
        keep: false,
        force: false,
        verbose: false,
        stdout: false,
    };

    run_cli(&args[1..], &config, program)
}
