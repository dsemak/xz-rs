//! Modern XZ compression utility
//!
//! A modern Rust implementation of the xz compression utility, compatible with
//! the original xz but with improved performance and user experience.

use std::process;

mod opts;

use opts::XzOpts;

use xz_cli::{format_error_for_stderr, run_cli};

const PROGRAM_NAME: &str = "xz";

fn main() -> std::io::Result<()> {
    let opts = XzOpts::parse();

    let config = match opts.config() {
        Ok(config) => config,
        Err(err) => {
            // Match upstream `xz`: `-qq` suppresses runtime error messages but does
            // not suppress clap's own argument parsing errors.
            if opts.quiet < 2 {
                eprintln!("{PROGRAM_NAME}: {err}");
            }

            process::exit(1);
        }
    };

    if let Err(err) = run_cli(&opts.files, &config, PROGRAM_NAME) {
        if let Some(msg) = format_error_for_stderr(PROGRAM_NAME, config.quiet, &err) {
            eprintln!("{msg}");
        }
        process::exit(1);
    }

    Ok(())
}
