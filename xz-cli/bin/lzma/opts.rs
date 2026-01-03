//! Command line argument parsing for the lzma utility.

use clap::Parser;

use xz_cli::{parse_memory_limit, CliConfig, OperationMode};

/// LZMA compression utility.
///
/// This is conceptually equivalent to `xz --format=lzma`.
#[derive(Parser, Debug)]
#[command(
    name = "lzma",
    version = "0.1.1",
    about = "Compress or decompress .lzma files",
    long_about = "lzma is equivalent to 'xz --format=lzma'. It supports streaming \
                 compression and decompression using the legacy .lzma container format."
)]
#[allow(clippy::struct_excessive_bools)]
pub struct LzmaOpts {
    /// Files to process
    #[arg(value_name = "FILE")]
    pub files: Vec<String>,

    /// Force compression
    #[arg(short = 'z', long = "compress", conflicts_with_all = ["decompress", "test"])]
    pub compress: bool,

    /// Force decompression
    #[arg(
        short = 'd',
        long = "decompress",
        alias = "uncompress",
        conflicts_with_all = ["compress", "test"]
    )]
    pub decompress: bool,

    /// Test compressed file integrity
    #[arg(short = 't', long = "test", conflicts_with_all = ["compress", "decompress"])]
    pub test: bool,

    /// Write to standard output and don't delete input files
    #[arg(short = 'c', long = "stdout", alias = "to-stdout")]
    pub stdout: bool,

    /// Force overwrite of output file
    #[arg(short = 'f', long = "force")]
    pub force: bool,

    /// Keep (don't delete) input files
    #[arg(short = 'k', long = "keep")]
    pub keep: bool,

    /// Verbose mode
    #[arg(short = 'v', long = "verbose", conflicts_with = "quiet")]
    pub verbose: bool,

    /// Quiet mode (suppress warnings). Use twice to suppress errors too.
    #[arg(short = 'q', long = "quiet", conflicts_with = "verbose", action = clap::ArgAction::Count)]
    pub quiet: u8,

    /// Compression preset level 0..9
    #[arg(short = '0', group = "level")]
    pub level_0: bool,
    #[arg(short = '1', group = "level")]
    pub level_1: bool,
    #[arg(short = '2', group = "level")]
    pub level_2: bool,
    #[arg(short = '3', group = "level")]
    pub level_3: bool,
    #[arg(short = '4', group = "level")]
    pub level_4: bool,
    #[arg(short = '5', group = "level")]
    pub level_5: bool,
    #[arg(short = '6', group = "level")]
    pub level_6: bool,
    #[arg(short = '7', group = "level")]
    pub level_7: bool,
    #[arg(short = '8', group = "level")]
    pub level_8: bool,
    #[arg(short = '9', group = "level")]
    pub level_9: bool,

    /// Use extreme compression (slower but better compression)
    #[arg(short = 'e', long = "extreme")]
    pub extreme: bool,

    /// LZMA1 encoder options.
    #[arg(long = "lzma1", value_name = "OPTS", num_args = 0..=1, default_missing_value = "")]
    pub lzma1: Option<String>,

    /// Use custom suffix on compressed files
    #[arg(short = 'S', long = "suffix", value_name = "SUFFIX")]
    pub suffix: Option<String>,

    /// Use at most this many threads (ignored for .lzma; kept for CLI compatibility)
    #[arg(short = 'T', long = "threads", value_name = "NUM")]
    pub threads: Option<usize>,

    /// Memory usage limit for decompression
    #[arg(
        short = 'M',
        long = "memory",
        alias = "memlimit",
        value_name = "LIMIT",
        value_parser = parse_memory_limit
    )]
    pub memory: Option<u64>,

    /// Decompress only the first stream, ignore remaining input
    #[arg(long = "single-stream")]
    pub single_stream: bool,

    /// Don't verify the integrity check when decompressing
    #[arg(long = "ignore-check")]
    pub ignore_check: bool,

    /// Don't create sparse files when decompressing.
    #[arg(long = "no-sparse")]
    pub no_sparse: bool,
}

impl LzmaOpts {
    /// Parse command line arguments.
    pub fn parse() -> Self {
        Parser::parse()
    }

    fn operation_mode(&self) -> OperationMode {
        if self.decompress {
            OperationMode::Decompress
        } else if self.test {
            OperationMode::Test
        } else {
            OperationMode::Compress
        }
    }

    fn compression_level(&self) -> Option<u32> {
        [
            (self.level_0, 0),
            (self.level_1, 1),
            (self.level_2, 2),
            (self.level_3, 3),
            (self.level_4, 4),
            (self.level_5, 5),
            (self.level_6, 6),
            (self.level_7, 7),
            (self.level_8, 8),
            (self.level_9, 9),
        ]
        .iter()
        .find_map(|&(flag, level)| flag.then_some(level))
    }

    /// Build CLI configuration from the parsed options.
    pub fn config(&self) -> CliConfig {
        CliConfig {
            mode: self.operation_mode(),
            force: self.force,
            keep: self.keep,
            stdout: self.stdout,
            verbose: self.verbose,
            quiet: self.quiet,
            level: self.compression_level(),
            threads: self.threads,
            memory_limit: self.memory,
            extreme: self.extreme,
            format: xz_core::config::DecodeMode::Lzma,
            check: xz_core::options::IntegrityCheck::None,
            lzma1: self.lzma1.clone(),
            robot: false,
            suffix: self.suffix.clone(),
            single_stream: self.single_stream,
            ignore_check: self.ignore_check,
            sparse: !self.no_sparse,
        }
    }
}
