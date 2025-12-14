//! Gzip compression utility
//!
//! A modern Rust implementation of the gzip compression utility.

use std::io;
use gzip_utils::{run_cli, CliConfig, OperationMode};

fn parse_args(args: &[String]) -> (CliConfig, Vec<String>) {
    let mut config = CliConfig {
        mode: OperationMode::Compress,
        level: Some(6),  // default compression level
        keep: false,
        force: false,
        verbose: false,
        stdout: false,
    };
    
    let mut files = Vec::new();
    let mut i = 0;
    
    while i < args.len() {
        match args[i].as_str() {
            "-f" | "--force" => config.force = true,
            "-k" | "--keep" => config.keep = true,
            "-v" | "--verbose" => config.verbose = true,
            "-c" | "--stdout" => config.stdout = true,
            "-d" | "--decompress" => config.mode = OperationMode::Decompress,
            "-t" | "--test" => config.mode = OperationMode::Test,
            "-l" | "--list" => {
                // Not implemented yet
                eprintln!("List mode not implemented");
            }
            arg if arg.starts_with('-') && arg.len() == 2 && arg.chars().nth(1).unwrap().is_digit(10) => {
                // Compression level: -1, -2, ..., -9
                if let Ok(level) = arg[1..].parse::<u32>() {
                    config.level = Some(level);
                }
            }
            "--fast" => config.level = Some(1),
            "--best" => config.level = Some(9),
            arg if !arg.starts_with('-') => {
                files.push(arg.to_string());
            }
            _ => {
                // Unknown option, treat as file (common in Unix tools)
                files.push(args[i].clone());
            }
        }
        i += 1;
    }
    
    (config, files)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = "gzip";
    
    let (config, files) = parse_args(&args[1..]);
    
    run_cli(&files, &config, program)
}
