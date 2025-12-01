//! Modern XZ compression utility
//!
//! A modern Rust implementation of the xz compression utility, compatible with
//! the original xz but with improved performance and user experience.

use std::process;

mod opts;

use opts::XzOpts;

use xz_cli::run_cli;

fn main() -> std::io::Result<()> {
    let opts = XzOpts::parse();

    let config = match opts.config() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("xz: {err}");
            process::exit(1);
        }
    };

    if let Err(err) = run_cli(&opts.files, &config, "xz") {
        eprintln!("xz: {err}");
        process::exit(1);
    }

    Ok(())
}
