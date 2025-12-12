//! Compression and decompression operations for XZ/LZMA CLI.

use std::io;
use lzma_rs::{lzma_compress, lzma_decompress, lzma2_compress, lzma2_decompress};
//use lzma_rs::compress::LzmaParams;

use crate::config::{CliConfig, CompressionFormat};
use crate::error::{Error, Result};

/// Compresses data using XZ or LZMA format
pub fn compress_file(
    mut input: impl io::Read,
    mut output: impl io::Write,
    config: &CliConfig,
) -> Result<()> {
    let mut input_data = Vec::new();
    io::copy(&mut input, &mut input_data)?;

    let mut output_data = Vec::new();

 //   let level = config.level.unwrap_or(6) as u32;
    match config.format {
        CompressionFormat::Xz => {
            // XZ format (LZMA2) with compression level
            lzma2_compress(&mut &input_data[..], &mut output_data)
                .map_err(|e| Error::Compression {
                    path: "(input)".to_string(),
                    message: format!("XZ compression failed: {}", e),
                })?;
        }
        CompressionFormat::Lzma => {
            lzma_compress(&mut &input_data[..], &mut output_data)
                .map_err(|e| Error::Compression {
                    path: "(input)".to_string(),
                    message: format!("LZMA compression failed: {}", e),
                })?;
        }
        CompressionFormat::Auto => {
            // Default to XZ format for auto
            lzma2_compress(&mut &input_data[..], &mut output_data)
                .map_err(|e| Error::Compression {
                    path: "(input)".to_string(),
                    message: format!("XZ compression failed: {}", e),
                })?;
        }
    }
    
    // Write compressed data to output
    io::copy(&mut &output_data[..], &mut output)?;

    if config.verbose {
        eprintln!("Compressed {} bytes to {} bytes (ratio: {:.1}%)", 
                 input_data.len(), 
                 output_data.len(),
                 if input_data.len() > 0 {
                     (output_data.len() as f64 / input_data.len() as f64) * 100.0
                 } else { 0.0 });
    }
    
    Ok(())
}

/// Decompresses XZ or LZMA data
pub fn decompress_file(
    mut input: impl io::Read,
    mut output: impl io::Write,
    config: &CliConfig,
) -> Result<()> {
    let mut input_data = Vec::new();
    io::copy(&mut input, &mut input_data)?;

     if input_data.is_empty() {
        return Err(Error::Decompression {
            path: "(input)".to_string(),
            message: "Empty input data".to_string(),
        });
    }
    
    let mut output_data = Vec::new();
    
    // Always try XZ first
    match lzma2_decompress(&mut &input_data[..], &mut output_data) {
        Ok(_) => {
            // Successfull decompressed as XZ
        }
        Err(e1) => {
            // Try LZMA
            output_data.clear();
            // If XZ didn't work, try LZMA
            lzma_decompress(&mut &input_data[..], &mut output_data)
                .map_err(|e2| {
                    // Both formats decompression failed
                    Error::Decompression {
                        path: "(input)".to_string(),
                        message: format!("Both XZ and LZMA decompression failed. XZ error: {}, LZMA error: {}", e1, e2),
                    }
                })?;
        }
    }

    io::copy(&mut &output_data[..], &mut output)?;

    if config.verbose {
        eprintln!("Decompressed {} bytes to {} bytes", 
                 input_data.len(), 
                 output_data.len());
    }

    Ok(())
}

// Detects compression format from magic bytes
//fn detect_format(data: &[u8]) -> Result<CompressionFormat> {
//    // XZ magic bytes: FD 37 7A 58 5A 00
//    if data.len() >= 6 && &data[0..6] == b"\xFD7zXZ\x00" {
//        return Ok(CompressionFormat::Xz);
//    }
//    
//    // LZMA format detection
//    // LZMA stream starts with specific properties byte
//    if data.len() >= 13 {
//        // Simplified LZMA detection - try decompression
//        // If not XZ, then LZMA
//        return Ok(CompressionFormat::Lzma);
//    }
//    
//    // Insufficient data to determine
//    Err(Error::FormatDetection {
//        path: "(input)".to_string(),
//        message: "Cannot determine compression format: file too small or corrupted".to_string(),
//    })
//}
