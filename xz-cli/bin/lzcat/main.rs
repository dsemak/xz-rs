//! LZMA/XZ decompression and concatenation utility
//!
//! This utility decompresses LZMA/XZ files and outputs the result to stdout,
//! similar to 'zcat' for gzip files. It can handle multiple files and
//! concatenate their decompressed content.

use std::process;

mod opts;

use opts::LzCatOpts;

use xz_cli::{format_diagnostic_for_stderr, run_cli};

const PROGRAM_NAME: &str = "lzcat";

fn main() {
    let opts = LzCatOpts::parse();
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
