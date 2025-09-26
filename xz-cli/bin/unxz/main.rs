//! XZ decompression utility
//!
//! Dedicated decompression and integrity testing utility for the XZ format.

mod opts;

use std::process;

use anyhow::Result;
use opts::UnxzOpts;
use xz_cli::run_cli;

fn main() -> Result<()> {
    let opts = UnxzOpts::parse();
    let config = opts.config();

    if let Err(err) = run_cli(opts.files(), &config, "unxz") {
        eprintln!("{err}");
        process::exit(1);
    }

    Ok(())
}
