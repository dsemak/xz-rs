use std::io::Cursor;
use std::path::{Path, PathBuf};

use xz_core::options::CompressionOptions;
use xz_core::pipeline::compress;

use super::*;

/// Test basic memory limit parsing with different units
#[test]
fn parse_memory_limit_basic_units() {
    assert_eq!(parse_memory_limit("1024").unwrap(), 1024);
    assert_eq!(parse_memory_limit("1K").unwrap(), 1024);
    assert_eq!(parse_memory_limit("1M").unwrap(), 1024 * 1024);
    assert_eq!(parse_memory_limit("1G").unwrap(), 1024 * 1024 * 1024);
}

/// Test case insensitivity for memory limit suffixes
#[test]
fn parse_memory_limit_case_insensitive() {
    assert_eq!(parse_memory_limit("512k").unwrap(), 512 * 1024);
    assert_eq!(parse_memory_limit("512K").unwrap(), 512 * 1024);
    assert_eq!(parse_memory_limit("2m").unwrap(), 2 * 1024 * 1024);
    assert_eq!(parse_memory_limit("2M").unwrap(), 2 * 1024 * 1024);
    assert_eq!(parse_memory_limit("1g").unwrap(), 1024 * 1024 * 1024);
    assert_eq!(parse_memory_limit("1G").unwrap(), 1024 * 1024 * 1024);
}

/// Test large valid memory limits
#[test]
fn parse_memory_limit_large_values() {
    assert_eq!(parse_memory_limit("1024M").unwrap(), 1024 * 1024 * 1024);
    assert_eq!(parse_memory_limit("16G").unwrap(), 16 * 1024 * 1024 * 1024);
}

/// Test invalid memory limit inputs
#[test]
fn parse_memory_limit_invalid_inputs() {
    assert!(parse_memory_limit("").is_err());
    assert!(parse_memory_limit("invalid").is_err());
    assert!(parse_memory_limit("1X").is_err());
    assert!(parse_memory_limit("1T").is_err());
    assert!(parse_memory_limit("-1K").is_err());
    assert!(parse_memory_limit("1.5M").is_err());
}

/// Test memory limit overflow detection
#[test]
fn parse_memory_limit_overflow() {
    // u64::MAX should overflow when multiplied
    let max_str = format!("{}G", u64::MAX);
    assert!(parse_memory_limit(&max_str).is_err());
}

/// Test edge case: zero value
#[test]
fn parse_memory_limit_zero() {
    assert_eq!(parse_memory_limit("0").unwrap(), 0);
    assert_eq!(parse_memory_limit("0K").unwrap(), 0);
}

/// Test recognition of valid compression extensions
#[test]
fn has_compression_extension_valid() {
    assert!(has_compression_extension(Path::new("file.xz")));
    assert!(has_compression_extension(Path::new("file.lzma")));
    assert!(has_compression_extension(Path::new("FILE.XZ")));
    assert!(has_compression_extension(Path::new("FILE.LZMA")));
    assert!(has_compression_extension(Path::new("archive.tar.xz")));
}

/// Test rejection of non-compression extensions
#[test]
fn has_compression_extension_invalid() {
    assert!(!has_compression_extension(Path::new("file.txt")));
    assert!(!has_compression_extension(Path::new("file.gz")));
    assert!(!has_compression_extension(Path::new("file.tar")));
    assert!(!has_compression_extension(Path::new("file")));
    assert!(!has_compression_extension(Path::new("file.xz.txt")));
}

/// Test paths without extensions
#[test]
fn has_compression_extension_no_extension() {
    assert!(!has_compression_extension(Path::new("filename")));
    assert!(!has_compression_extension(Path::new("/path/to/file")));
}

/// Test compression output filename generation
#[test]
fn generate_output_filename_compress_basic() {
    let input = Path::new("test.txt");
    let output =
        generate_output_filename(input, OperationMode::Compress, None, XZ_EXTENSION, false)
            .unwrap();
    assert_eq!(output, PathBuf::from("test.txt.xz"));

    let input = Path::new("test");
    let output =
        generate_output_filename(input, OperationMode::Compress, None, XZ_EXTENSION, false)
            .unwrap();
    assert_eq!(output, PathBuf::from("test.xz"));
}

/// Test compression with existing .xz extension
#[test]
fn generate_output_filename_compress_double_extension() {
    let input = Path::new("file.tar");
    let output =
        generate_output_filename(input, OperationMode::Compress, None, XZ_EXTENSION, false)
            .unwrap();
    assert_eq!(output, PathBuf::from("file.tar.xz"));
}

/// Test compression with paths
#[test]
fn generate_output_filename_compress_with_path() {
    let input = Path::new("/path/to/file.txt");
    let output =
        generate_output_filename(input, OperationMode::Compress, None, XZ_EXTENSION, false)
            .unwrap();
    assert_eq!(output, PathBuf::from("/path/to/file.txt.xz"));
}

/// Test decompression output filename generation
#[test]
fn generate_output_filename_decompress_basic() {
    let input = Path::new("test.txt.xz");
    let output =
        generate_output_filename(input, OperationMode::Decompress, None, XZ_EXTENSION, false)
            .unwrap();
    assert_eq!(output, PathBuf::from("test.txt"));

    let input = Path::new("test.lzma");
    let output =
        generate_output_filename(input, OperationMode::Decompress, None, XZ_EXTENSION, false)
            .unwrap();
    assert_eq!(output, PathBuf::from("test"));
}

/// Test decompression with paths
#[test]
fn generate_output_filename_decompress_with_path() {
    let input = Path::new("/path/to/archive.xz");
    let output =
        generate_output_filename(input, OperationMode::Decompress, None, XZ_EXTENSION, false)
            .unwrap();
    assert_eq!(output, PathBuf::from("/path/to/archive"));
}

/// Test Cat mode output filename generation (same as decompress)
#[test]
fn generate_output_filename_cat_mode() {
    let input = Path::new("test.txt.xz");
    let output =
        generate_output_filename(input, OperationMode::Cat, None, XZ_EXTENSION, false).unwrap();
    assert_eq!(output, PathBuf::from("test.txt"));
}

/// Test Test mode returns empty path
#[test]
fn generate_output_filename_test_mode() {
    let input = Path::new("test.xz");
    let output =
        generate_output_filename(input, OperationMode::Test, None, XZ_EXTENSION, false).unwrap();
    assert_eq!(output, PathBuf::new());
}

/// Test decompression with invalid extension fails
#[test]
fn generate_output_filename_decompress_invalid_extension() {
    let input = Path::new("test.txt");
    let result =
        generate_output_filename(input, OperationMode::Decompress, None, XZ_EXTENSION, false);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        DiagnosticCause::Warning(Warning::InvalidExtension { .. })
    ));
}

/// Test decompression with no extension fails
#[test]
fn generate_output_filename_decompress_no_extension() {
    let input = Path::new("test");
    let result =
        generate_output_filename(input, OperationMode::Decompress, None, XZ_EXTENSION, false);
    assert!(result.is_err());
}

/// Test compression with custom suffix
#[test]
fn generate_output_filename_compress_custom_suffix() {
    let input = Path::new("test.txt");
    let output = generate_output_filename(
        input,
        OperationMode::Compress,
        Some("myext"),
        XZ_EXTENSION,
        false,
    )
    .unwrap();
    assert_eq!(output, PathBuf::from("test.txt.myext"));

    let input = Path::new("file");
    let output = generate_output_filename(
        input,
        OperationMode::Compress,
        Some("gz"),
        XZ_EXTENSION,
        false,
    )
    .unwrap();
    assert_eq!(output, PathBuf::from("file.gz"));
}

/// Test compression with custom suffix that includes dot
#[test]
fn generate_output_filename_compress_custom_suffix_with_dot() {
    let input = Path::new("test.txt");
    let output = generate_output_filename(
        input,
        OperationMode::Compress,
        Some(".custom"),
        XZ_EXTENSION,
        false,
    )
    .unwrap();
    // Leading dot should be stripped, so we get .custom not ..custom
    assert_eq!(output, PathBuf::from("test.txt.custom"));
}

/// Test decompression with custom suffix
#[test]
fn generate_output_filename_decompress_custom_suffix() {
    let input = Path::new("test.txt.myext");
    let output = generate_output_filename(
        input,
        OperationMode::Decompress,
        Some("myext"),
        XZ_EXTENSION,
        false,
    )
    .unwrap();
    assert_eq!(output, PathBuf::from("test.txt"));

    let input = Path::new("file.custom");
    let output = generate_output_filename(
        input,
        OperationMode::Decompress,
        Some(".custom"),
        XZ_EXTENSION,
        false,
    )
    .unwrap();
    assert_eq!(output, PathBuf::from("file"));
}

/// Test decompression with custom suffix fails on wrong extension
#[test]
fn generate_output_filename_decompress_custom_suffix_mismatch() {
    let input = Path::new("test.txt.xz");
    let result = generate_output_filename(
        input,
        OperationMode::Decompress,
        Some("myext"),
        XZ_EXTENSION,
        false,
    );
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DiagnosticCause::Warning(Warning::InvalidExtension { .. }))
    ));
}

/// Test compression fails when file already has the target suffix
#[test]
fn generate_output_filename_compress_already_has_suffix() {
    let input = Path::new("test.txt.xz");
    let result =
        generate_output_filename(input, OperationMode::Compress, None, XZ_EXTENSION, false);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DiagnosticCause::Warning(Warning::AlreadyHasSuffix { .. }))
    ));

    let input = Path::new("test.custom");
    let result = generate_output_filename(
        input,
        OperationMode::Compress,
        Some("custom"),
        XZ_EXTENSION,
        false,
    );
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DiagnosticCause::Warning(Warning::AlreadyHasSuffix { .. }))
    ));

    let input = Path::new("test.myext");
    let result = generate_output_filename(
        input,
        OperationMode::Compress,
        Some(".myext"),
        XZ_EXTENSION,
        false,
    );
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(DiagnosticCause::Warning(Warning::AlreadyHasSuffix { .. }))
    ));
}

/// Test compression with force flag allows files with target suffix
#[test]
fn generate_output_filename_compress_force_allows_suffix() {
    let input = Path::new("test.txt.xz");
    let output =
        generate_output_filename(input, OperationMode::Compress, None, XZ_EXTENSION, true).unwrap();
    assert_eq!(output, PathBuf::from("test.txt.xz.xz"));

    let input = Path::new("test.custom");
    let output = generate_output_filename(
        input,
        OperationMode::Compress,
        Some("custom"),
        XZ_EXTENSION,
        true,
    )
    .unwrap();
    assert_eq!(output, PathBuf::from("test.custom.custom"));
}

/// Test [`CliConfig`] default values
#[test]
fn cli_config_defaults() {
    let config = CliConfig::default();
    assert_eq!(config.mode, OperationMode::Compress);
    assert!(!config.force);
    assert!(!config.keep);
    assert!(!config.stdout);
    assert!(!config.verbose);
    assert_eq!(config.level, None);
    assert_eq!(config.threads, None);
    assert_eq!(config.memory_limit, None);
}

/// Test [`OperationMode`] derives
#[test]
fn operation_mode_traits() {
    let mode1 = OperationMode::Compress;
    let mode2 = OperationMode::Compress;
    let mode3 = OperationMode::Decompress;

    assert_eq!(mode1, mode2);
    assert_ne!(mode1, mode3);

    // Test Copy
    let mode_copy = mode1;
    assert_eq!(mode1, mode_copy);

    // Test Debug
    let debug_str = format!("{mode1:?}");
    assert!(debug_str.contains("Compress"));
}

/// Test compression round-trip
#[test]
fn compress_decompress_roundtrip() {
    use std::io::Cursor;

    let original_data = b"The quick brown fox jumps over the lazy dog";
    let mut compressed = Vec::new();

    // Compress
    let compress_config = CliConfig {
        mode: OperationMode::Compress,
        level: Some(6),
        ..Default::default()
    };

    compress_file(
        Cursor::new(original_data),
        &mut compressed,
        &compress_config,
    )
    .expect("Compression should succeed");

    assert!(
        !compressed.is_empty(),
        "Compressed data should not be empty"
    );

    // Decompress
    let mut decompressed = Vec::new();
    let decompress_config = CliConfig {
        mode: OperationMode::Decompress,
        ..Default::default()
    };

    decompress_file(
        Cursor::new(&compressed),
        &mut decompressed,
        &decompress_config,
    )
    .expect("Decompression should succeed");

    assert_eq!(
        decompressed, original_data,
        "Decompressed data should match original"
    );
}

/// Test empty input compression
#[test]
fn compress_empty_input() {
    use std::io::Cursor;

    let empty_data: &[u8] = b"";
    let mut output_vec = Vec::new();
    let config = CliConfig::default();

    compress_file(Cursor::new(empty_data), &mut output_vec, &config)
        .expect("Compressing empty input should succeed");

    assert!(
        !output_vec.is_empty(),
        "Empty input should produce XZ header"
    );
}

/// Test compression with different levels
#[test]
fn compression_levels() {
    use std::io::Cursor;

    let data = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit.";

    for level in 0..=9 {
        let mut output_vec = Vec::new();
        let config = CliConfig {
            mode: OperationMode::Compress,
            level: Some(level),
            ..Default::default()
        };

        compress_file(Cursor::new(data), &mut output_vec, &config)
            .unwrap_or_else(|_| panic!("Compression with level {level} should succeed"));

        assert!(
            !output_vec.is_empty(),
            "Level {level} should produce output"
        );
    }
}

/// Test invalid compression level
#[test]
fn invalid_compression_level() {
    use std::io::Cursor;

    let data = b"test";
    let mut output_vec = Vec::new();
    let config = CliConfig {
        mode: OperationMode::Compress,
        level: Some(300), // Invalid level
        ..Default::default()
    };

    let err = compress_file(Cursor::new(data), &mut output_vec, &config).unwrap_err();
    assert!(matches!(
        err,
        DiagnosticCause::Error(Error::InvalidCompressionLevel { level: 300 })
    ));
}

/// Test decompression with corrupted data
#[test]
fn decompress_corrupted_data() {
    use std::io::Cursor;

    let corrupted_data = b"This is not valid XZ data";
    let mut output_vec = Vec::new();
    let config = CliConfig::default();

    let err = decompress_file(Cursor::new(corrupted_data), &mut output_vec, &config).unwrap_err();
    assert!(matches!(
        err,
        DiagnosticCause::Error(Error::Decompression { .. })
    ));
}

/// Test verbose flag behavior
#[test]
fn verbose_output() {
    use std::io::Cursor;

    let data = b"test data for verbose output";
    let mut output_vec = Vec::new();
    let config = CliConfig {
        mode: OperationMode::Compress,
        verbose: true,
        ..Default::default()
    };

    // This should not panic even with verbose enabled
    compress_file(Cursor::new(data), &mut output_vec, &config)
        .expect("Compression with verbose should succeed");
}

/// Test extreme mode applies to default level (6)
#[test]
fn extreme_mode_default_level() {
    use std::io::Cursor;

    let data = b"Test data for compression";
    let mut output = Vec::new();

    let config = CliConfig {
        extreme: true,
        ..Default::default()
    };

    compress_file(Cursor::new(data), &mut output, &config).unwrap();

    assert!(!output.is_empty());

    // Verify decompression works
    let mut decompressed = Vec::new();
    decompress_file(
        Cursor::new(&output),
        &mut decompressed,
        &CliConfig::default(),
    )
    .unwrap();

    assert_eq!(&decompressed[..], data);
}

/// Test extreme mode is a modifier, not level 9
#[test]
fn extreme_mode_is_modifier_not_level9() {
    use std::io::Cursor;

    let data = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(50);

    // Compress with -0e (extreme level 0)
    let mut output_0e = Vec::new();
    compress_file(
        Cursor::new(&data),
        &mut output_0e,
        &CliConfig {
            level: Some(0),
            extreme: true,
            ..Default::default()
        },
    )
    .unwrap();

    // Compress with -9e (extreme level 9)
    let mut output_9e = Vec::new();
    compress_file(
        Cursor::new(&data),
        &mut output_9e,
        &CliConfig {
            level: Some(9),
            extreme: true,
            ..Default::default()
        },
    )
    .unwrap();

    // If extreme was just "level 9", both would produce identical results
    // But since extreme is a modifier, -0e and -9e should differ
    assert_ne!(output_0e.len(), output_9e.len(),);
}

/// Test thread count conversion
#[test]
fn thread_count_edge_cases() {
    use std::io::Cursor;

    let data = b"test";
    let mut output_vec = Vec::new();

    // Test thread count 1
    let config = CliConfig {
        mode: OperationMode::Compress,
        threads: Some(1),
        ..Default::default()
    };

    compress_file(Cursor::new(data), &mut output_vec, &config)
        .expect("Single-threaded compression should work");
}

/// Test memory limit with decompression
#[test]
fn memory_limit_decompression() {
    use std::io::Cursor;

    // First compress some data
    let data = b"test data";
    let mut compressed = Vec::new();
    let compress_config = CliConfig::default();

    compress_file(Cursor::new(data), &mut compressed, &compress_config)
        .expect("Compression should succeed");

    // Then decompress with a reasonable memory limit
    let mut output_vec = Vec::new();
    let decompress_config = CliConfig {
        mode: OperationMode::Decompress,
        memory_limit: Some(64 * 1024 * 1024), // 64MB
        ..Default::default()
    };

    decompress_file(
        Cursor::new(&compressed),
        &mut output_vec,
        &decompress_config,
    )
    .expect("Decompression with memory limit should succeed");

    assert_eq!(output_vec, data);
}

/// Test that zero memory limit is handled correctly
#[test]
fn zero_memory_limit() {
    use std::io::Cursor;

    let data = b"test";
    let mut compressed = Vec::new();

    compress_file(Cursor::new(data), &mut compressed, &CliConfig::default())
        .expect("Compression should succeed");

    let mut output = Vec::new();
    let config = CliConfig {
        mode: OperationMode::Decompress,
        memory_limit: Some(0), // Zero limit - should be ignored
        ..Default::default()
    };

    decompress_file(Cursor::new(&compressed), &mut output, &config)
        .expect("Zero memory limit should not cause failure");
}

/// Test constants are defined correctly
#[test]
fn constants_have_expected_values() {
    assert_eq!(XZ_EXTENSION, "xz");
    assert_eq!(LZMA_EXTENSION, "lzma");
    assert_eq!(DEFAULT_BUFFER_SIZE, 512 * 1024);
}

fn compress_to_xz_bytes(plain: &[u8]) -> Vec<u8> {
    let opts = CompressionOptions::default();
    let mut out = Vec::new();
    match compress(plain, &mut out, &opts) {
        Ok(summary) => {
            assert!(summary.bytes_written > 0);
        }
        Err(err) => panic!("compress failed: {err:?}"),
    }
    out
}

/// Default CLI behavior (without `--single-stream`) decodes concatenated `.xz` streams fully.
#[test]
fn default_decodes_concatenated_xz_streams() {
    let plain_a = b"hello from stream A\n";
    let plain_b = b"and this is stream B\n";

    let xz_a = compress_to_xz_bytes(plain_a);
    let xz_b = compress_to_xz_bytes(plain_b);

    let mut concatenated = Vec::with_capacity(xz_a.len() + xz_b.len());
    concatenated.extend_from_slice(&xz_a);
    concatenated.extend_from_slice(&xz_b);

    let config = CliConfig {
        single_stream: false, // default, but make it explicit for test clarity
        ..Default::default()
    };

    let mut decoded = Vec::new();
    match decompress_file(Cursor::new(concatenated), &mut decoded, &config) {
        Ok(()) => {}
        Err(err) => panic!("decompress_file(default) failed: {err:?}"),
    }

    let mut expected = Vec::with_capacity(plain_a.len() + plain_b.len());
    expected.extend_from_slice(plain_a);
    expected.extend_from_slice(plain_b);
    assert_eq!(decoded, expected);
}

/// `--single-stream` stops after the first stream and ignores remaining input without error.
#[test]
fn single_stream_ignores_trailing_stream() {
    let plain_a = b"hello from stream A\n";
    let plain_b = b"and this is stream B\n";

    let xz_a = compress_to_xz_bytes(plain_a);
    let xz_b = compress_to_xz_bytes(plain_b);

    let mut concatenated = Vec::with_capacity(xz_a.len() + xz_b.len());
    concatenated.extend_from_slice(&xz_a);
    concatenated.extend_from_slice(&xz_b);

    let config = CliConfig {
        single_stream: true,
        ..Default::default()
    };

    let mut decoded = Vec::new();
    match decompress_file(Cursor::new(concatenated), &mut decoded, &config) {
        Ok(()) => {}
        Err(err) => panic!("decompress_file(--single-stream) failed: {err:?}"),
    }

    assert_eq!(decoded, plain_a);
}
