//! Modern XZ compression utility
//!
//! A modern Rust implementation of the xz compression utility, compatible with
//! the original xz but with improved performance and user experience.

use std::process;

mod opts;

use opts::XzOpts;

use xz_cli::{argfiles, CliError, Error, InvocationError, Result};
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

    let files = match resolve_input_files(&opts) {
        Ok(files) => files,
        Err(err) => {
            let err = InvocationError::new(err, PROGRAM_NAME, None);
            if let Some(msg) = format_error_for_stderr(config.quiet, &err) {
                eprintln!("{msg}");
            }
            process::exit(1);
        }
    };

    if let Err(err) = run_cli(&files, &config, PROGRAM_NAME) {
        if let Some(msg) = format_error_for_stderr(config.quiet, &err) {
            eprintln!("{msg}");
        }
        process::exit(1);
    }

    Ok(())
}

fn resolve_input_files(opts: &XzOpts) -> Result<Vec<String>> {
    let mut files = opts.files.clone();

    if let Some(path) = opts.files_from_file.as_deref() {
        let extra =
            argfiles::read_files(Some(path), argfiles::Delimiter::Line).map_err(|source| {
                CliError::Error(Error::OpenInput {
                    path: path.to_string(),
                    source,
                })
            })?;
        files.extend(extra);
    }

    if let Some(path) = opts.files0_from_file.as_deref() {
        let extra =
            argfiles::read_files(Some(path), argfiles::Delimiter::Nul).map_err(|source| {
                CliError::Error(Error::OpenInput {
                    path: path.to_string(),
                    source,
                })
            })?;
        files.extend(extra);
    }

    Ok(files)
}
