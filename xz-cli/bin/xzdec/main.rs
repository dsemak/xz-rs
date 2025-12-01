//! Small XZ decompression utility
//!
//! A minimal decompression-only utility for XZ files that serves as a drop-in
//! replacement for xz --decompress --stdout in common scenarios.

use std::process;

mod opts;

use opts::XzDecOpts;

use xz_cli::run_cli;

fn main() -> std::io::Result<()> {
    let opts = XzDecOpts::parse();
    let config = opts.config();

    if let Err(err) = run_cli(opts.files(), &config, "xzdec") {
        if !opts.is_quiet() {
            eprintln!("xzdec: {err}");
        }
        process::exit(1);
    }

    Ok(())
}
