//! LZMA decompression utility
//!
//! This utility decompresses LZMA files, serving as a dedicated decompression
//! tool for the LZMA format. It's equivalent to 'lzma -d' but provides a
//! more convenient interface for decompression-only operations.

use std::process;

mod opts;

use opts::UnlzmaOpts;

use xz_cli::{format_diagnostic_for_stderr, run_cli};

const PROGRAM_NAME: &str = "unlzma";

fn main() {
    let opts = UnlzmaOpts::parse();
    let config = opts.config();

    let report = run_cli(opts.files(), &config, PROGRAM_NAME);
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
