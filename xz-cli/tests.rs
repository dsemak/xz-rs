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
    let output = generate_output_filename(input, OperationMode::Compress).unwrap();
    assert_eq!(output, PathBuf::from("test.txt.xz"));

    let input = Path::new("test");
    let output = generate_output_filename(input, OperationMode::Compress).unwrap();
    assert_eq!(output, PathBuf::from("test.xz"));
}

/// Test compression with existing .xz extension
#[test]
fn generate_output_filename_compress_double_extension() {
    let input = Path::new("file.tar");
    let output = generate_output_filename(input, OperationMode::Compress).unwrap();
    assert_eq!(output, PathBuf::from("file.tar.xz"));
}

/// Test compression with paths
#[test]
fn generate_output_filename_compress_with_path() {
    let input = Path::new("/path/to/file.txt");
    let output = generate_output_filename(input, OperationMode::Compress).unwrap();
    assert_eq!(output, PathBuf::from("/path/to/file.txt.xz"));
}

/// Test decompression output filename generation
#[test]
fn generate_output_filename_decompress_basic() {
    let input = Path::new("test.txt.xz");
    let output = generate_output_filename(input, OperationMode::Decompress).unwrap();
    assert_eq!(output, PathBuf::from("test.txt"));

    let input = Path::new("test.lzma");
    let output = generate_output_filename(input, OperationMode::Decompress).unwrap();
    assert_eq!(output, PathBuf::from("test"));
}

/// Test decompression with paths
#[test]
fn generate_output_filename_decompress_with_path() {
    let input = Path::new("/path/to/archive.xz");
    let output = generate_output_filename(input, OperationMode::Decompress).unwrap();
    assert_eq!(output, PathBuf::from("/path/to/archive"));
}

/// Test Cat mode output filename generation (same as decompress)
#[test]
fn generate_output_filename_cat_mode() {
    let input = Path::new("test.txt.xz");
    let output = generate_output_filename(input, OperationMode::Cat).unwrap();
    assert_eq!(output, PathBuf::from("test.txt"));
}

/// Test Test mode returns empty path
#[test]
fn generate_output_filename_test_mode() {
    let input = Path::new("test.xz");
    let output = generate_output_filename(input, OperationMode::Test).unwrap();
    assert_eq!(output, PathBuf::new());
}

/// Test decompression with invalid extension fails
#[test]
fn generate_output_filename_decompress_invalid_extension() {
    let input = Path::new("test.txt");
    let result = generate_output_filename(input, OperationMode::Decompress);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
}

/// Test decompression with no extension fails
#[test]
fn generate_output_filename_decompress_no_extension() {
    let input = Path::new("test");
    let result = generate_output_filename(input, OperationMode::Decompress);
    assert!(result.is_err());
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
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

/// Test decompression with corrupted data
#[test]
fn decompress_corrupted_data() {
    use std::io::Cursor;

    let corrupted_data = b"This is not valid XZ data";
    let mut output_vec = Vec::new();
    let config = CliConfig::default();

    let err = decompress_file(Cursor::new(corrupted_data), &mut output_vec, &config).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
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
