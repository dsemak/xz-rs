//! Simple XZ decompression utility
//!
//! A minimal decompression-only utility for XZ files.

use std::io;
use std::process;

use anyhow::{Context, Result};
use xz_core::{options::DecompressionOptions, pipeline::decompress};

fn main() {
    if let Err(err) = run() {
        eprintln!("xzdec: {}", err);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    // xzdec is a minimal utility - no command line arguments
    // It always reads from stdin and writes to stdout
    let mut input = io::stdin();
    let mut output = io::stdout();

    // Use default decompression options with reasonable limits
    let options = DecompressionOptions::default();

    let _summary = decompress(&mut input, &mut output, &options).context("Decompression failed")?;

    Ok(())
}
