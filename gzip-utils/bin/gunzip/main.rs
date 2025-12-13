//! Gunzip decompression utility  
//!
//! A modern Rust implementation of the gunzip decompression utility.

use std::io;
use gzip_utils::{run_cli, CliConfig, OperationMode};

fn parse_args(args: &[String]) -> (CliConfig, Vec<String>) {
    let mut config = CliConfig {
        mode: OperationMode::Decompress,
        keep: false,
        force: false,
        verbose: false,
        stdout: false,
        level: None,
    };
    
    let mut files = Vec::new();
    
    for arg in args {
        match arg.as_str() {
            "-f" | "--force" => config.force = true,
            "-k" | "--keep" => config.keep = true,
            "-v" | "--verbose" => config.verbose = true,
            "-c" | "--stdout" => config.stdout = true,
            "-t" | "--test" => config.mode = OperationMode::Test,
            "-l" | "--list" => {
                eprintln!("List mode not implemented");
            }
            _ => {
                files.push(arg.clone());
            }
        }
    }
    
    (config, files)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = "gunzip";
    
    let (config, files) = parse_args(&args[1..]);
    
    run_cli(&files, &config, program)
}
