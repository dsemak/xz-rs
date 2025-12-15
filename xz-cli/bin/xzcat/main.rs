//! XZ decompression and concatenation utility
//!
//! This utility decompresses XZ files and outputs the result to stdout,
//! similar to 'zcat' for gzip files. It can handle multiple files and
//! concatenate their decompressed content.

use std::process;

mod opts;

use opts::XzCatOpts;

use xz_cli::{format_error_for_stderr, run_cli};

const PROGRAM_NAME: &str = "xzcat";

fn main() -> std::io::Result<()> {
    let opts = XzCatOpts::parse();
    let config = opts.config();

    if let Err(err) = run_cli(opts.files(), &config, PROGRAM_NAME) {
        if let Some(msg) = format_error_for_stderr(config.quiet, &err) {
            eprintln!("{msg}");
        }

        process::exit(1);
    }

    Ok(())
}
