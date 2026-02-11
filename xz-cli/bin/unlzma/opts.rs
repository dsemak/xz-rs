//! Command line argument parsing for the unlzma utility.

use clap::Parser;

use xz_cli::{parse_memory_limit, CliConfig, OperationMode};

/// LZMA decompression utility.
///
/// Equivalent to `lzma --decompress`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser)]
#[command(
    name = "unlzma",
    version = "0.1.1",
    about = "Decompress .lzma files",
    long_about = "unlzma is equivalent to 'lzma --decompress'. It decompresses files \
                 created by lzma and removes the .lzma suffix from the filename."
)]
pub struct UnlzmaOpts {
    /// Files to decompress
    #[arg(value_name = "FILE")]
    files: Vec<String>,

    /// Write to standard output and don't delete input files
    #[arg(short = 'c', long = "stdout", alias = "to-stdout")]
    stdout: bool,

    /// Force overwrite of output file
    #[arg(short = 'f', long = "force")]
    force: bool,

    /// Keep (don't delete) input files
    #[arg(short = 'k', long = "keep")]
    keep: bool,

    /// Verbose mode
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Quiet mode (suppress warnings). Use twice to suppress errors too.
    #[arg(short = 'q', long = "quiet", conflicts_with = "verbose", action = clap::ArgAction::Count)]
    pub quiet: u8,

    /// Test compressed file integrity
    #[arg(short = 't', long = "test")]
    test: bool,

    /// Use at most this many threads (ignored for .lzma; kept for CLI compatibility)
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

    /// Use custom suffix on compressed files
    #[arg(short = 'S', long = "suffix", value_name = "SUFFIX")]
    suffix: Option<String>,

    /// Don't create sparse files when decompressing.
    #[arg(long = "no-sparse")]
    no_sparse: bool,
}

impl UnlzmaOpts {
    /// Parse command line arguments.
    pub fn parse() -> Self {
        Parser::parse()
    }

    /// Build CLI configuration from the parsed options.
    pub fn config(&self) -> CliConfig {
        let mode = if self.test {
            OperationMode::Test
        } else {
            OperationMode::Decompress
        };

        CliConfig {
            mode,
            force: self.force,
            keep: self.keep,
            stdout: self.stdout,
            verbose: self.verbose,
            quiet: self.quiet,
            no_warn: false,
            level: None,
            threads: self.threads,
            memory_limit: self.memory,
            extreme: false,
            format: xz_core::config::DecodeMode::Lzma,
            check: xz_core::options::IntegrityCheck::None,
            lzma1: None,
            robot: false,
            suffix: self.suffix.clone(),
            single_stream: false,
            ignore_check: false,
            sparse: !self.no_sparse,
        }
    }

    /// Files supplied on the command line.
    pub fn files(&self) -> &[String] {
        &self.files
    }
}
