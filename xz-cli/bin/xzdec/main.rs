//! Small XZ decompression utility
//!
//! A minimal decompression-only utility for XZ files that serves as a drop-in
//! replacement for xz --decompress --stdout in common scenarios.

use std::process;

mod opts;

use opts::XzDecOpts;

use xz_cli::{format_error_for_stderr, run_cli};

const PROGRAM_NAME: &str = "xzdec";

fn main() -> std::io::Result<()> {
    let opts = XzDecOpts::parse();
    let config = opts.config();

    if let Err(err) = run_cli(opts.files(), &config, PROGRAM_NAME) {
        if let Some(msg) = format_error_for_stderr(config.quiet, &err) {
            eprintln!("{msg}");
        }

        process::exit(1);
    }

    Ok(())
}
