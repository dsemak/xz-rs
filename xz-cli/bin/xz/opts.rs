//! Command line argument parsing for xz utility

use clap::Parser;

use xz_cli::{parse_memory_limit, CliConfig, OperationMode};

/// Modern XZ compression utility
///
/// A modern Rust implementation of the xz compression utility, compatible with
/// the original xz but with improved performance and user experience.
#[derive(Parser, Debug)]
#[command(
    name = "xz",
    version = "0.1.1",
    about = "Compress or decompress .xz files",
    long_about = "xz is a general-purpose data compression tool with command line syntax \
                 similar to gzip and bzip2. The native file format is the .xz format."
)]
#[allow(clippy::struct_excessive_bools)]
pub struct XzOpts {
    /// Files to process
    #[arg(value_name = "FILE")]
    pub files: Vec<String>,

    /// Force compression
    #[arg(short = 'z', long = "compress", conflicts_with_all = ["decompress", "test"])]
    pub compress: bool,

    /// Force decompression
    #[arg(short = 'd', long = "decompress", conflicts_with_all = ["compress", "test"])]
    pub decompress: bool,

    /// Test compressed file integrity
    #[arg(short = 't', long = "test", conflicts_with_all = ["compress", "decompress"])]
    pub test: bool,

    /// Write to standard output and don't delete input files
    #[arg(short = 'c', long = "stdout")]
    pub stdout: bool,

    /// Force overwrite of output file
    #[arg(short = 'f', long = "force")]
    pub force: bool,

    /// Keep (don't delete) input files
    #[arg(short = 'k', long = "keep")]
    pub keep: bool,

    /// Verbose mode
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Compression level (1-9)
    #[arg(
        short = '1',
        short = '2',
        short = '3',
        short = '4',
        short = '5',
        short = '6',
        short = '7',
        short = '8',
        short = '9',
        value_name = "LEVEL",
        value_parser = clap::value_parser!(u8).range(1..=9)
    )]
    pub level: Option<u8>,

    /// Use at most this many threads
    #[arg(short = 'T', long = "threads", value_name = "NUM")]
    pub threads: Option<usize>,

    /// Memory usage limit for decompression
    #[arg(short = 'M', long = "memory", value_name = "LIMIT", value_parser = parse_memory_limit)]
    pub memory: Option<u64>,
}

impl XzOpts {
    /// Parse command line arguments
    pub fn parse() -> Self {
        Parser::parse()
    }

    /// Determine operation mode based on flags
    pub fn operation_mode(&self) -> OperationMode {
        if self.decompress {
            OperationMode::Decompress
        } else if self.test {
            OperationMode::Test
        } else if self.compress {
            OperationMode::Compress
        } else {
            // Auto-detect based on file extensions or default to compress
            OperationMode::Compress
        }
    }

    /// Build CLI configuration from the parsed options
    pub fn config(&self) -> CliConfig {
        CliConfig {
            mode: self.operation_mode(),
            force: self.force,
            keep: self.keep,
            stdout: self.stdout,
            verbose: self.verbose,
            level: self.level.map(u32::from),
            threads: self.threads,
            memory_limit: self.memory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test operation mode detection
    #[test]
    fn test_operation_mode() {
        let opts = XzOpts {
            files: vec![],
            compress: false,
            decompress: true,
            test: false,
            stdout: false,
            force: false,
            keep: false,
            verbose: false,
            level: None,
            threads: None,
            memory: None,
        };
        assert_eq!(opts.operation_mode(), OperationMode::Decompress);
        let config = opts.config();
        assert_eq!(config.mode, OperationMode::Decompress);
        assert!(!config.force);

        let opts = XzOpts {
            files: vec![],
            compress: false,
            decompress: false,
            test: true,
            stdout: false,
            force: false,
            keep: false,
            verbose: false,
            level: None,
            threads: None,
            memory: None,
        };
        assert_eq!(opts.operation_mode(), OperationMode::Test);
        let config = opts.config();
        assert_eq!(config.mode, OperationMode::Test);
        assert!(!config.stdout);
    }
}
