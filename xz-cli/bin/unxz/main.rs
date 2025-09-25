//! XZ decompression utility
//!
//! This utility decompresses XZ files, serving as a dedicated decompression
//! tool for the XZ format. It's equivalent to 'xz -d' but provides a
//! more convenient interface for decompression-only operations.

use std::process;

use anyhow::Result;
use clap::{Arg, ArgAction, Command};
use xz_cli::{process_file, CliConfig, OperationMode};

fn main() {
    if let Err(err) = run() {
        eprintln!("unxz: {}", err);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let matches = Command::new("unxz")
        .version("0.1.1")
        .about("Decompress .xz files")
        .long_about(
            "unxz is equivalent to 'xz --decompress'. It decompresses files \
                    created by xz and removes the .xz suffix from the filename.",
        )
        .arg(
            Arg::new("files")
                .help("Files to decompress")
                .value_name("FILE")
                .num_args(0..)
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("stdout")
                .short('c')
                .long("stdout")
                .help("Write to standard output and don't delete input files")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("force")
                .short('f')
                .long("force")
                .help("Force overwrite of output file")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("keep")
                .short('k')
                .long("keep")
                .help("Keep (don't delete) input files")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Verbose mode")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("test")
                .short('t')
                .long("test")
                .help("Test compressed file integrity")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("threads")
                .short('T')
                .long("threads")
                .help("Use at most this many threads")
                .value_name("NUM")
                .value_parser(clap::value_parser!(usize))
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("memory")
                .short('M')
                .long("memory")
                .help("Memory usage limit for decompression")
                .value_name("LIMIT")
                .value_parser(parse_memory_limit)
                .action(ArgAction::Set),
        )
        .get_matches();

    let mut config = CliConfig::default();

    // unxz always decompresses (or tests)
    config.mode = if matches.get_flag("test") {
        OperationMode::Test
    } else {
        OperationMode::Decompress
    };

    // Set configuration flags
    config.force = matches.get_flag("force");
    config.keep = matches.get_flag("keep");
    config.stdout = matches.get_flag("stdout");
    config.verbose = matches.get_flag("verbose");

    // Set thread count
    if let Some(threads) = matches.get_one::<usize>("threads") {
        config.threads = Some(*threads);
    }

    // Set memory limit
    if let Some(memory) = matches.get_one::<u64>("memory") {
        config.memory_limit = Some(*memory);
    }

    // Get input files
    let files: Vec<&String> = matches
        .get_many::<String>("files")
        .map(|vals| vals.collect())
        .unwrap_or_default();

    // Process files
    if files.is_empty() {
        // Process stdin
        process_file("-", &config)?;
    } else {
        for file in files {
            if let Err(err) = process_file(file, &config) {
                eprintln!("unxz: {}: {}", file, err);
                process::exit(1);
            }
        }
    }

    Ok(())
}

/// Parse memory limit with optional suffix (K, M, G)
fn parse_memory_limit(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty memory limit".to_string());
    }

    let (number_part, multiplier) = if let Some(last_char) = s.chars().last() {
        match last_char.to_ascii_uppercase() {
            'K' => (&s[..s.len() - 1], 1024_u64),
            'M' => (&s[..s.len() - 1], 1024_u64 * 1024),
            'G' => (&s[..s.len() - 1], 1024_u64 * 1024 * 1024),
            _ if last_char.is_ascii_digit() => (s, 1),
            _ => return Err(format!("Invalid memory limit suffix: {}", last_char)),
        }
    } else {
        (s, 1)
    };

    let number: u64 = number_part
        .parse()
        .map_err(|_| format!("Invalid memory limit number: {}", number_part))?;

    number
        .checked_mul(multiplier)
        .ok_or_else(|| "Memory limit too large".to_string())
}
