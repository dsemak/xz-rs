//! XZ compression utility
//!
//! A pure-Rust implementation of the xz compression utility.

use std::io;
use xz_utils::{run_cli, CliConfig, CompressionFormat, OperationMode};

fn parse_args(args: &[String]) -> (CliConfig, Vec<String>) {
    let mut config = CliConfig {
        mode: OperationMode::Compress,
        format: CompressionFormat::Xz,
        level: Some(6),
        keep: false,
        force: false,
        verbose: false,
        stdout: false,
    };
    
    let mut files = Vec::new();
    
    for arg in args {
        match arg.as_str() {
            "-f" | "--force" => config.force = true,
            "-k" | "--keep" => config.keep = true,
            "-v" | "--verbose" => config.verbose = true,
            "-c" | "--stdout" => config.stdout = true,
            "-d" | "--decompress" => config.mode = OperationMode::Decompress,
            "-t" | "--test" => config.mode = OperationMode::Test,
            "--format=xz" => config.format = CompressionFormat::Xz,
            "--format=lzma" => config.format = CompressionFormat::Lzma,
            "--format=auto" => config.format = CompressionFormat::Auto,
            arg if arg.starts_with('-') && arg.len() == 2 && arg.chars().nth(1).unwrap().is_digit(10) => {
                if let Ok(level) = arg[1..].parse::<u32>() {
                    config.level = Some(level);
                }
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
    let program = "xz";
    
    let (config, files) = parse_args(&args[1..]);
    
    run_cli(&files, &config, program)
}
