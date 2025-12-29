//! LZMA compression and decompression utility
//!
//! This utility provides LZMA compression and decompression functionality,
//! serving as a legacy compatibility layer for the older LZMA format.
//! It supports both compression and decompression operations.

use std::process;

mod opts;

use opts::LzmaOpts;

use xz_cli::{format_diagnostic_for_stderr, run_cli};

const PROGRAM_NAME: &str = "lzma";

fn main() {
    let opts = LzmaOpts::parse();
    let config = opts.config();

    let report = run_cli(&opts.files, &config, PROGRAM_NAME);
    for diagnostic in &report.diagnostics {
        if let Some(msg) = format_diagnostic_for_stderr(config.quiet, diagnostic) {
            eprintln!("{msg}");
        }
    }
    let code = report.status.code();
    if code != 0 {
        process::exit(code);
    }
}
