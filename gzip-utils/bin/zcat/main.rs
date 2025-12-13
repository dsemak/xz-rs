//! XZ decompression and concatenation utility
//!
//! This utility decompresses XZ files and outputs the result to stdout,
//! similar to 'zcat' for gzip files. It can handle multiple files and
//! concatenate their decompressed content.

use std::io;
use gzip_utils::{run_cli, CliConfig, OperationMode};

fn parse_args(args: &[String]) -> (CliConfig, Vec<String>) {
    let mut config = CliConfig {
        mode: OperationMode::Cat,
        stdout: true,    // Always output to stdout
        keep: true,      // Never remove input files
        force: false,
        verbose: false,
        level: None,
    };
    
    let mut files = Vec::new();
    
    for arg in args {
        match arg.as_str() {
            "-v" | "--verbose" => config.verbose = true,
            _ => {
                files.push(arg.clone());
            }
        }
    }
    
    (config, files)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = "zcat";
    
    let (config, files) = parse_args(&args[1..]);
    
    run_cli(&files, &config, program)
}
