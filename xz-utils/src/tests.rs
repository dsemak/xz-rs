use std::io::Cursor;

use super::*;

/// Test basic compression/decompression roundtrip with XZ format
#[test]
fn xz_roundtrip() {
    let data = b"Test data for XZ compression";
    let mut compressed = Vec::new();
    let mut decompressed = Vec::new();

    let compress_config = CliConfig {
        mode: OperationMode::Compress,
        format: CompressionFormat::Xz,
        level: Some(6),
        ..Default::default()
    };

    // Compress
    compress_file(
        Cursor::new(data),
        &mut compressed, 
        &compress_config
    ).unwrap();

    assert!(!compressed.is_empty(), "Compressed data should not be empty");

    let decompress_config = CliConfig {
        mode: OperationMode::Decompress,
        format: CompressionFormat::Auto,
        ..Default::default()
    };

    // Decompress
    decompress_file(
        Cursor::new(&compressed),
        &mut decompressed,
        &decompress_config
    ).unwrap();

    assert_eq!(decompressed, data, "Decompressed data should match original");
}

/// Test LZMA format compression/decompression
#[test]
fn lzma_roundtrip() {
    let data = b"Test data for LZMA compression";
    let mut compressed = Vec::new();
    let mut decompressed = Vec::new();

    let compress_config = CliConfig {
        mode: OperationMode::Compress,
        format: CompressionFormat::Lzma,
        level: Some(5),
        ..Default::default()
    };

    // Compress
    compress_file(
        Cursor::new(data),
        &mut compressed, 
        &compress_config
    ).unwrap();

    assert!(!compressed.is_empty());

    let decompress_config = CliConfig {
        mode: OperationMode::Decompress,
        format: CompressionFormat::Lzma,
        ..Default::default()
    };

    // Decompress
    decompress_file(
        Cursor::new(&compressed),
        &mut decompressed,
        &decompress_config
    ).unwrap();

    assert_eq!(decompressed, data);
}

/// Test extension recognition
#[test]
fn extension_recognition() {
    use std::path::Path;

    assert!(has_compression_extension(Path::new("file.xz")));
    assert!(has_compression_extension(Path::new("file.lzma")));
    assert!(has_compression_extension(Path::new("FILE.XZ")));
    assert!(has_compression_extension(Path::new("FILE.LZMA")));
    assert!(!has_compression_extension(Path::new("file.gz")));
    assert!(!has_compression_extension(Path::new("file.txt")));
}

/// Test output filename generation for compression
#[test]
fn generate_output_filename_compress() {
    use std::path::Path;

    let config = CliConfig {
        mode: OperationMode::Compress,
        format: CompressionFormat::Xz,
        ..Default::default()
    };

    let input = Path::new("test.txt");
    let output = generate_output_filename(input, &config).unwrap();
    assert_eq!(output, Path::new("test.txt.xz"));

    let config_lzma = CliConfig {
        mode: OperationMode::Compress,
        format: CompressionFormat::Lzma,
        ..Default::default()
    };

    let output_lzma = generate_output_filename(input, &config_lzma).unwrap();
    assert_eq!(output_lzma, Path::new("test.txt.lzma"));
}

/// Test output filename generation for decompression
#[test]
fn generate_output_filename_decompress() {
    use std::path::Path;

    let config = CliConfig {
        mode: OperationMode::Decompress,
        format: CompressionFormat::Auto,
        ..Default::default()
    };

    let input = Path::new("archive.tar.xz");
    let output = generate_output_filename(input, &config).unwrap();
    assert_eq!(output, Path::new("archive.tar"));

    let input_lzma = Path::new("data.bin.lzma");
    let output_lzma = generate_output_filename(input_lzma, &config).unwrap();
    assert_eq!(output_lzma, Path::new("data.bin"));
}
