//! Command line argument parsing for the unxz utility.

use clap::Parser;

use xz_cli::{parse_memory_limit, CliConfig, OperationMode};

/// XZ decompression utility
///
/// Equivalent to `xz --decompress`. Can optionally test integrity without
/// writing the decompressed output.
#[derive(Debug, Parser)]
#[command(
    name = "unxz",
    version = "0.1.1",
    about = "Decompress .xz files",
    long_about = "unxz is equivalent to 'xz --decompress'. It decompresses files \
                 created by xz and removes the .xz suffix from the filename."
)]
pub struct UnxzOpts {
    /// Files to decompress
    #[arg(value_name = "FILE")]
    files: Vec<String>,

    /// Write to standard output and don't delete input files
    #[arg(short = 'c', long = "stdout")]
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

    /// Quiet mode (suppress warnings)
    #[arg(short = 'q', long = "quiet", conflicts_with = "verbose")]
    pub quiet: bool,

    /// Test compressed file integrity
    #[arg(short = 't', long = "test")]
    test: bool,

    /// Use at most this many threads
    #[arg(short = 'T', long = "threads", value_name = "NUM")]
    threads: Option<usize>,

    /// Memory usage limit for decompression
    #[arg(short = 'M', long = "memory", value_name = "LIMIT", value_parser = parse_memory_limit)]
    memory: Option<u64>,
}

impl UnxzOpts {
    /// Parse command line arguments
    pub fn parse() -> Self {
        Parser::parse()
    }

    /// Build CLI configuration from the parsed options
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
            level: None,
            threads: self.threads,
            memory_limit: self.memory,
            extreme: false,
            format: xz_core::config::DecodeMode::Auto,
            check: xz_core::options::IntegrityCheck::Crc64,
            robot: false,
        }
    }

    /// Files supplied on the command line
    pub fn files(&self) -> &[String] {
        &self.files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_reflects_test_mode() {
        let opts = UnxzOpts {
            files: vec!["test.xz".into()],
            stdout: false,
            force: true,
            keep: false,
            verbose: true,
            quiet: false,
            test: true,
            threads: Some(8),
            memory: Some(1024),
        };

        let config = opts.config();
        assert_eq!(config.mode, OperationMode::Test);
        assert!(config.force);
        assert!(config.verbose);
        assert_eq!(config.threads, Some(8));
        assert_eq!(config.memory_limit, Some(1024));
    }

    #[test]
    fn parse_from_args_sets_flags() {
        let opts =
            UnxzOpts::try_parse_from(["unxz", "-cvk", "-T", "4", "-M", "1M", "file.xz"]).unwrap();

        assert_eq!(opts.files(), ["file.xz"]);
        assert!(opts.stdout);
        assert!(opts.keep);
        assert!(opts.verbose);
        assert_eq!(opts.threads, Some(4));
        assert_eq!(opts.memory, Some(1024 * 1024));
    }
}
