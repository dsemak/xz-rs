//! Small XZ decompression utility
//!
//! A minimal decompression-only utility for XZ files that serves as a drop-in
//! replacement for xz --decompress --stdout in common scenarios.

use std::process;

mod opts;

use opts::XzDecOpts;

use xz_cli::{format_diagnostic_for_stderr, run_cli};

const PROGRAM_NAME: &str = "xzdec";

fn main() -> std::io::Result<()> {
    let opts = XzDecOpts::parse();
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

    Ok(())
}
