//! XZ decompression and concatenation utility
//!
//! This utility decompresses XZ files and outputs the result to stdout,
//! similar to 'zcat' for gzip files. It can handle multiple files and
//! concatenate their decompressed content.

use std::io;
use gzip_utils::{run_cli, CliConfig, OperationMode};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = "zcat";

    let config = CliConfig {
        mode: OperationMode::Cat,
        stdout: true,    // Always output to stdout
        keep: true,      // Never remove input files
        force: false,
        verbose: false,
        level: None,    // Not used for decompression
    };

    run_cli(&args[1..], &config, program)
}
