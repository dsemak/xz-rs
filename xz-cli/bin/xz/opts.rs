//! Command line argument parsing for xz utility

use clap::Parser;

use xz_cli::{parse_memory_limit, CliConfig, OperationMode};
use xz_core::{config::DecodeMode, options::IntegrityCheck};

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
    #[arg(short = 'z', long = "compress", conflicts_with_all = ["decompress", "test", "list"])]
    pub compress: bool,

    /// Force decompression
    #[arg(
        short = 'd',
        long = "decompress",
        alias = "uncompress",
        conflicts_with_all = ["compress", "test", "list"]
    )]
    pub decompress: bool,

    /// Test compressed file integrity
    #[arg(short = 't', long = "test", conflicts_with_all = ["compress", "decompress", "list"])]
    pub test: bool,

    /// List information about compressed files
    #[arg(short = 'l', long = "list", conflicts_with_all = ["compress", "decompress", "test"])]
    pub list: bool,

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

    /// Compression preset level 0 (no compression, fastest)
    #[arg(short = '0', group = "level")]
    pub level_0: bool,

    /// Compression preset level 1 (fastest)
    #[arg(short = '1', group = "level")]
    pub level_1: bool,

    /// Compression preset level 2
    #[arg(short = '2', group = "level")]
    pub level_2: bool,

    /// Compression preset level 3
    #[arg(short = '3', group = "level")]
    pub level_3: bool,

    /// Compression preset level 4
    #[arg(short = '4', group = "level")]
    pub level_4: bool,

    /// Compression preset level 5
    #[arg(short = '5', group = "level")]
    pub level_5: bool,

    /// Compression preset level 6 (default)
    #[arg(short = '6', group = "level")]
    pub level_6: bool,

    /// Compression preset level 7
    #[arg(short = '7', group = "level")]
    pub level_7: bool,

    /// Compression preset level 8
    #[arg(short = '8', group = "level")]
    pub level_8: bool,

    /// Compression preset level 9 (best)
    #[arg(short = '9', group = "level")]
    pub level_9: bool,

    /// Use at most this many threads
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

    /// Use extreme compression (slower but better compression)
    #[arg(short = 'e', long = "extreme")]
    pub extreme: bool,

    /// File format to use
    #[arg(short = 'F', long = "format", value_name = "FORMAT")]
    pub format: Option<String>,

    /// Integrity check type
    #[arg(short = 'C', long = "check", value_name = "TYPE")]
    pub check: Option<String>,

    /// Read filenames from file (one per line)
    #[arg(
        long = "files",
        value_name = "FILE",
        num_args = 0..=1,
        default_missing_value = "-",
        conflicts_with = "files0_from_file"
    )]
    pub files_from_file: Option<String>,

    /// Read filenames from file (null-terminated)
    #[arg(
        long = "files0",
        value_name = "FILE",
        num_args = 0..=1,
        default_missing_value = "-",
        conflicts_with = "files_from_file"
    )]
    pub files0_from_file: Option<String>,

    /// Machine-readable output
    #[arg(long = "robot")]
    pub robot: bool,

    /// Use custom suffix on compressed files
    #[arg(short = 'S', long = "suffix", value_name = "SUFFIX")]
    pub suffix: Option<String>,

    /// Decompress only the first stream, ignore remaining input
    #[arg(long = "single-stream")]
    pub single_stream: bool,

    /// Don't verify the integrity check when decompressing
    #[arg(long = "ignore-check")]
    pub ignore_check: bool,

    /// Don't create sparse files when decompressing.
    ///
    /// Upstream `xz` attempts to create sparse output files by turning long runs
    /// of zero bytes into holes. Use this option to always write the zero bytes
    /// instead.
    #[arg(long = "no-sparse")]
    pub no_sparse: bool,

    /// Display long help and exit
    #[arg(short = 'H', long = "long-help", action = clap::ArgAction::Help)]
    _long_help: Option<bool>,
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
        } else if self.list {
            OperationMode::List
        } else if self.compress {
            OperationMode::Compress
        } else {
            // Auto-detect based on file extensions or default to compress
            OperationMode::Compress
        }
    }

    /// Parse the file format from the format string
    pub fn file_format(&self) -> Result<DecodeMode, Box<dyn std::error::Error>> {
        match self.format.as_deref() {
            Some("xz") => Ok(DecodeMode::Xz),
            Some("lzma") => Ok(DecodeMode::Lzma),
            Some("raw" | "auto") | None => Ok(DecodeMode::Auto), // Raw format is handled differently
            Some(invalid) => Err(format!("{invalid}: Unknown file format type").into()),
        }
    }

    /// Parse the check type from the check string
    pub fn check_type(&self) -> Result<IntegrityCheck, Box<dyn std::error::Error>> {
        match self.check.as_deref() {
            Some("none") => Ok(IntegrityCheck::None),
            Some("crc32") => Ok(IntegrityCheck::Crc32),
            Some("sha256") => Ok(IntegrityCheck::Sha256),
            Some("crc64") | None => Ok(IntegrityCheck::Crc64),
            Some(invalid) => Err(format!("{invalid}: Unsupported integrity check type").into()),
        }
    }

    /// Get the compression level from the preset flags
    pub fn compression_level(&self) -> Option<u8> {
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

    /// Build CLI configuration from the parsed options
    pub fn config(&self) -> Result<CliConfig, Box<dyn std::error::Error>> {
        Ok(CliConfig {
            mode: self.operation_mode(),
            force: self.force,
            keep: self.keep,
            stdout: self.stdout,
            verbose: self.verbose,
            quiet: self.quiet,
            level: self.compression_level().map(u32::from),
            threads: self.threads,
            memory_limit: self.memory,
            extreme: self.extreme,
            format: self.file_format()?,
            check: self.check_type()?,
            robot: self.robot,
            suffix: self.suffix.clone(),
            single_stream: self.single_stream,
            ignore_check: self.ignore_check,
            sparse: !self.no_sparse,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to create default [`XzOpts`] for testing
    fn default_opts() -> XzOpts {
        XzOpts {
            files: vec![],
            compress: false,
            decompress: false,
            test: false,
            list: false,
            stdout: false,
            force: false,
            keep: false,
            verbose: false,
            quiet: 0,
            level_0: false,
            level_1: false,
            level_2: false,
            level_3: false,
            level_4: false,
            level_5: false,
            level_6: false,
            level_7: false,
            level_8: false,
            level_9: false,
            threads: None,
            memory: None,
            extreme: false,
            format: None,
            check: None,
            files_from_file: None,
            files0_from_file: None,
            robot: false,
            suffix: None,
            single_stream: false,
            ignore_check: false,
            no_sparse: false,
            _long_help: None,
        }
    }

    /// Test operation mode detection
    #[test]
    fn test_operation_mode() {
        let opts = XzOpts {
            decompress: true,
            ..default_opts()
        };
        assert_eq!(opts.operation_mode(), OperationMode::Decompress);
        let config = opts.config().unwrap();
        assert_eq!(config.mode, OperationMode::Decompress);
        assert!(!config.force);

        let opts = XzOpts {
            test: true,
            ..default_opts()
        };
        assert_eq!(opts.operation_mode(), OperationMode::Test);
        let config = opts.config().unwrap();
        assert_eq!(config.mode, OperationMode::Test);
        assert!(!config.stdout);
    }

    /// Test compression level detection from preset flags
    #[test]
    fn test_compression_level() {
        let mut opts = default_opts();

        // Test no level set
        assert_eq!(opts.compression_level(), None);

        // Test each level
        opts.level_1 = true;
        assert_eq!(opts.compression_level(), Some(1));
        opts.level_1 = false;

        opts.level_5 = true;
        assert_eq!(opts.compression_level(), Some(5));
        opts.level_5 = false;

        opts.level_9 = true;
        assert_eq!(opts.compression_level(), Some(9));
    }

    #[test]
    fn parse_accepts_aliases() {
        let opts = match XzOpts::try_parse_from([
            "xz",
            "--uncompress",
            "--to-stdout",
            "--memlimit",
            "1M",
            "file.xz",
        ]) {
            Ok(v) => v,
            Err(e) => panic!("failed to parse aliases: {e}"),
        };

        assert!(opts.decompress);
        assert!(opts.stdout);
        assert_eq!(opts.memory, Some(1024 * 1024));
        assert_eq!(opts.files, ["file.xz"]);
    }
}
