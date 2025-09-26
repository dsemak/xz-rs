//! Simple XZ decompression utility.
//!
//! A minimal decompression-only utility for XZ files. Reads from stdin, writes to stdout.

use std::io;

use xz_core::{options::DecompressionOptions, pipeline::decompress};

fn main() -> io::Result<()> {
    let mut input = io::stdin();
    let mut output = io::stdout();

    let options = DecompressionOptions::default();

    decompress(&mut input, &mut output, &options)
        .map(|_| ())
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}
