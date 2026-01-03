//! Command line argument parsing for the lzcat utility.

use clap::Parser;

use xz_cli::{parse_memory_limit, CliConfig, OperationMode};

/// LZMA decompression and concatenation utility.
///
/// Equivalent to `lzma --decompress --stdout`.
#[derive(Debug, Parser)]
#[command(
    name = "lzcat",
    version = "0.1.1",
    about = "Decompress .lzma files to stdout",
    long_about = "lzcat decompresses files and writes the output to standard output. \
                 It is equivalent to 'lzma --decompress --stdout'. Multiple files \
                 are decompressed and concatenated to stdout."
)]
pub struct LzCatOpts {
    /// Files to decompress
    #[arg(value_name = "FILE")]
    files: Vec<String>,

    /// Verbose mode
    #[arg(short = 'v', long = "verbose", conflicts_with = "quiet")]
    verbose: bool,

    /// Quiet mode (suppress warnings). Use twice to suppress errors too.
    #[arg(short = 'q', long = "quiet", conflicts_with = "verbose", action = clap::ArgAction::Count)]
    quiet: u8,

    /// Use at most this many threads
    #[arg(short = 'T', long = "threads", value_name = "NUM")]
    threads: Option<usize>,

    /// Memory usage limit for decompression
    #[arg(
        short = 'M',
        long = "memory",
        alias = "memlimit",
        value_name = "LIMIT",
        value_parser = parse_memory_limit
    )]
    memory: Option<u64>,
}

impl LzCatOpts {
    /// Parse command line arguments.
    pub fn parse() -> Self {
        Parser::parse()
    }

    /// Build CLI configuration from the parsed options.
    pub fn config(&self) -> CliConfig {
        CliConfig {
            mode: OperationMode::Cat,
            force: false,
            keep: true,
            stdout: true,
            verbose: self.verbose,
            quiet: self.quiet,
            level: None,
            threads: self.threads,
            memory_limit: self.memory,
            extreme: false,
            format: xz_core::config::DecodeMode::Auto,
            check: xz_core::options::IntegrityCheck::None,
            lzma1: None,
            robot: false,
            suffix: None,
            single_stream: false,
            ignore_check: false,
            sparse: false,
        }
    }

    /// Files supplied on the command line.
    pub fn files(&self) -> &[String] {
        &self.files
    }
}
