//! Modern XZ compression utility
//!
//! A modern Rust implementation of the xz compression utility, compatible with
//! the original xz but with improved performance and user experience.

mod opts;

use std::process;

use anyhow::Result;
use xz_cli::run_cli;

use opts::XzOpts;

fn main() -> Result<()> {
    let opts = XzOpts::parse();

    let config = opts.config();

    if let Err(err) = run_cli(&opts.files, &config, "xz") {
        eprintln!("{err}");
        process::exit(1);
    }

    Ok(())
}
