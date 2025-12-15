//! Command line argument parsing for the xzcat utility.

use clap::Parser;

use xz_cli::{parse_memory_limit, CliConfig, OperationMode};

/// XZ decompression and concatenation utility
///
/// This utility decompresses XZ files and writes the output to standard output.
#[derive(Debug, Parser)]
#[command(
    name = "xzcat",
    version = "0.1.1",
    about = "Decompress .xz files to stdout",
    long_about = "xzcat decompresses files and writes the output to standard output. \
                 It is equivalent to 'xz --decompress --stdout'. Multiple files \
                 are decompressed and concatenated to stdout."
)]
pub struct XzCatOpts {
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

impl XzCatOpts {
    /// Parse command line arguments
    pub fn parse() -> Self {
        Parser::parse()
    }

    /// Build CLI configuration from the parsed options
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
            check: xz_core::options::IntegrityCheck::Crc64,
            robot: false,
            suffix: None,
            single_stream: false,
            ignore_check: false,
            sparse: false,
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
    fn config_sets_cat_mode_and_stdout() {
        let opts = XzCatOpts {
            files: vec!["input.xz".into()],
            verbose: true,
            quiet: 0,
            threads: Some(4),
            memory: Some(1024),
        };

        let config = opts.config();
        assert_eq!(config.mode, OperationMode::Cat);
        assert!(config.stdout);
        assert!(config.keep);
        assert!(config.verbose);
        assert_eq!(config.threads, Some(4));
        assert_eq!(config.memory_limit, Some(1024));
    }

    #[test]
    fn parse_from_args_reads_flags() {
        let opts = XzCatOpts::try_parse_from(["xzcat", "-v", "-T", "2", "-M", "512K", "input.xz"])
            .unwrap();

        assert_eq!(opts.files(), ["input.xz"]);
        assert!(opts.verbose);
        assert_eq!(opts.threads, Some(2));
        assert_eq!(opts.memory, Some(512 * 1024));
    }

    #[test]
    fn parse_accepts_memlimit_alias() {
        let opts = match XzCatOpts::try_parse_from(["xzcat", "--memlimit", "1M", "input.xz"]) {
            Ok(v) => v,
            Err(e) => panic!("failed to parse aliases: {e}"),
        };

        assert_eq!(opts.files(), ["input.xz"]);
        assert_eq!(opts.memory, Some(1024 * 1024));
    }
}
