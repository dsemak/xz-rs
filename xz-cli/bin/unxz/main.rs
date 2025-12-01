//! XZ decompression utility
//!
//! This utility decompresses XZ files, serving as a dedicated decompression
//! tool for the XZ format. It's equivalent to 'xz -d' but provides a
//! more convenient interface for decompression-only operations.

use std::process;

mod opts;

use opts::UnxzOpts;

use xz_cli::run_cli;

fn main() -> std::io::Result<()> {
    let opts = UnxzOpts::parse();
    let config = opts.config();

    if let Err(err) = run_cli(opts.files(), &config, "unxz") {
        eprintln!("unxz: {err}");
        process::exit(1);
    }

    Ok(())
}
