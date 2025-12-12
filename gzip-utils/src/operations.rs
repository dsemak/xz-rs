//! Compression and decompression operations for CLI.

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io;

use crate::config::CliConfig;
use crate::error::{Error, Result};

/// Compresses data using gzip format
pub fn compress_file(
    mut input: impl io::Read,
    output: impl io::Write,
    config: &CliConfig,
) -> Result<()> {
    let level = config.level.unwrap_or(6).clamp(0, 9);
    let compression = Compression::new(level);

    let mut encoder = GzEncoder::new(output, compression);
    let bytes_read = io::copy(&mut input, &mut encoder).map_err(|e| Error::Compression {
        path: "(input)".to_string(),
        message: e.to_string(),
    })?;

    encoder.finish().map_err(|e| Error::Compression {
        path: "(input)".to_string(),
        message: e.to_string(),
    })?;

    if config.verbose {
        eprintln!("Compressed {} bytes", bytes_read);
    }

    Ok(())
}

/// Decompresses gzip data
pub fn decompress_file(
    input: impl io::Read,
    mut output: impl io::Write,
    config: &CliConfig,
) -> Result<()> {
    let mut decoder = GzDecoder::new(input);
    let bytes_written = io::copy(&mut decoder, &mut output).map_err(|e| Error::Decompression {
        path: "(input)".to_string(),
        message: e.to_string(),
    })?;

    if config.verbose {
        eprintln!("Decompressed {} bytes", bytes_written);
    }

    Ok(())
}
