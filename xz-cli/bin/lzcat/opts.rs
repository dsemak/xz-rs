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
            format: xz_core::config::DecodeMode::Lzma,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify [`LzCatOpts::config`] uses the legacy `.lzma` container mode.
    #[test]
    fn config_uses_lzma_decode_mode() {
        let opts = LzCatOpts {
            files: vec!["input.lzma".into()],
            verbose: false,
            quiet: 0,
            threads: Some(4),
            memory: Some(1024),
        };

        let config = opts.config();
        assert_eq!(config.mode, OperationMode::Cat);
        assert!(config.stdout);
        assert_eq!(config.format, xz_core::config::DecodeMode::Lzma);
    }

    /// Ensure `--memlimit` alias is accepted (upstream CLI compatibility).
    #[test]
    fn parse_accepts_memlimit_alias() {
        let opts = match LzCatOpts::try_parse_from(["lzcat", "--memlimit", "1M", "input.lzma"]) {
            Ok(v) => v,
            Err(e) => panic!("failed to parse aliases: {e}"),
        };

        assert_eq!(opts.files(), ["input.lzma"]);
        assert_eq!(opts.memory, Some(1024 * 1024));
    }
}
