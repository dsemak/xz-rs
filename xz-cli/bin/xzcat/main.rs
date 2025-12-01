//! XZ decompression and concatenation utility
//!
//! This utility decompresses XZ files and outputs the result to stdout,
//! similar to 'zcat' for gzip files. It can handle multiple files and
//! concatenate their decompressed content.

use std::process;

mod opts;

use opts::XzCatOpts;

use xz_cli::run_cli;

fn main() -> std::io::Result<()> {
    let opts = XzCatOpts::parse();
    let config = opts.config();

    if let Err(err) = run_cli(opts.files(), &config, "xzcat") {
        eprintln!("xzcat: {err}");
        process::exit(1);
    }

    Ok(())
}
