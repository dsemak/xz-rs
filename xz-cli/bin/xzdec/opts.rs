//! Command line argument parsing for the xzdec utility.

use clap::Parser;

use xz_cli::{parse_memory_limit, CliConfig, OperationMode};

/// Small .xz decompressor
///
/// xzdec is a liblzma-based decompression-only tool for .xz (and only .xz) files.
/// xzdec is intended to work as a drop-in replacement for xz(1) in the most common
/// situations where a script has been written to use xz --decompress --stdout.
#[derive(Debug, Parser)]
#[command(
    name = "xzdec",
    version = "0.1.1",
    about = "Small .xz decompressor",
    long_about = "xzdec is a liblzma-based decompression-only tool for .xz (and only .xz) files. \
                 xzdec is intended to work as a drop-in replacement for xz(1) in the most common \
                 situations where a script has been written to use xz --decompress --stdout."
)]
pub struct XzDecOpts {
    /// Files to decompress
    #[arg(value_name = "FILE")]
    files: Vec<String>,

    /// Ignored for xz(1) compatibility. xzdec supports only decompression.
    #[arg(short = 'd', long = "decompress", alias = "uncompress")]
    _decompress: bool,

    /// Ignored for xz(1) compatibility. xzdec never creates or removes any files.
    #[arg(short = 'k', long = "keep")]
    _keep: bool,

    /// Ignored for xz(1) compatibility. xzdec always writes the decompressed data to standard output.
    #[arg(short = 'c', long = "stdout", alias = "to-stdout")]
    _stdout: bool,

    /// Memory usage limit for decompression
    #[arg(short = 'M', long = "memory", value_name = "LIMIT", value_parser = parse_memory_limit)]
    memory: Option<u64>,

    /// Suppress errors when specified twice
    #[arg(short = 'q', long = "quiet", action = clap::ArgAction::Count)]
    quiet: u8,

    /// Ignored for xz(1) compatibility. xzdec never uses the exit status 2.
    #[arg(short = 'Q', long = "no-warn")]
    _no_warn: bool,
}

impl XzDecOpts {
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
            verbose: false,
            quiet: false,
            level: None,
            threads: None,
            memory_limit: self.memory,
            extreme: false,
            format: xz_core::config::DecodeMode::Auto,
            check: xz_core::options::IntegrityCheck::Crc64,
            robot: false,
            suffix: None,
        }
    }

    /// Files supplied on the command line
    pub fn files(&self) -> &[String] {
        &self.files
    }

    /// Check if quiet mode is enabled (suppress errors when -q specified twice)
    pub fn is_quiet(&self) -> bool {
        self.quiet >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test basic configuration
    #[test]
    fn config_sets_cat_mode_and_stdout() {
        let opts = XzDecOpts {
            files: vec!["input.xz".into()],
            _decompress: false,
            _keep: false,
            _stdout: false,
            memory: Some(1024),
            quiet: 0,
            _no_warn: false,
        };

        let config = opts.config();
        assert_eq!(config.mode, OperationMode::Cat);
        assert!(config.stdout);
        assert!(config.keep);
        assert!(!config.verbose);
        assert_eq!(config.memory_limit, Some(1024));
    }

    /// Test memory limit parsing
    #[test]
    fn parse_from_args_reads_memory_limit() {
        let opts = XzDecOpts::try_parse_from(["xzdec", "-M", "512K", "input.xz"]).unwrap();

        assert_eq!(opts.files(), ["input.xz"]);
        assert_eq!(opts.memory, Some(512 * 1024));
        assert!(!opts.is_quiet());
    }

    /// Test quiet mode
    #[test]
    fn quiet_mode_requires_double_q() {
        let opts = XzDecOpts::try_parse_from(["xzdec", "-q", "input.xz"]).unwrap();
        assert!(!opts.is_quiet());

        let opts = XzDecOpts::try_parse_from(["xzdec", "-qq", "input.xz"]).unwrap();
        assert!(opts.is_quiet());
    }

    /// Test compatibility options are ignored
    #[test]
    fn compatibility_options_are_ignored() {
        let opts =
            XzDecOpts::try_parse_from(["xzdec", "-d", "-k", "-c", "-Q", "input.xz"]).unwrap();

        assert_eq!(opts.files(), ["input.xz"]);
        // These options should be parsed but ignored in behavior
        assert!(opts._decompress);
        assert!(opts._keep);
        assert!(opts._stdout);
        assert!(opts._no_warn);
    }
}
