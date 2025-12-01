use crate::add_test;
use crate::common::{Fixture, BINARY_DATA, REPETITIVE_DATA, SAMPLE_TEXT};

// Test basic compression and decompression
add_test!(compress_decompress, async {
    const FILE_NAME: &str = "input.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));

    fixture.remove_file(FILE_NAME);

    // Decompress
    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test compression with -k (keep) flag
add_test!(compress_keep_original, async {
    const FILE_NAME: &str = "keep_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    // Both files should exist
    assert!(fixture.file_exists(FILE_NAME));
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});

// Test compression with -f (force) flag
add_test!(compress_force_overwrite, async {
    const FILE_NAME: &str = "force_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // First compression
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Recreate original file
    let mut fixture = Fixture::with_file(FILE_NAME, data);
    let file_path = fixture.path(FILE_NAME);

    // Force overwrite
    let output = fixture.run_cargo("xz", &["-f", &file_path]).await;
    assert!(output.status.success());
});

// Test compression with -c (stdout) flag
add_test!(compress_to_stdout, async {
    const FILE_NAME: &str = "stdout_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    let output = fixture.run_cargo("xz", &["-c", &file_path]).await;
    assert!(output.status.success());

    // Original file should still exist
    assert!(fixture.file_exists(FILE_NAME));

    // stdout should contain compressed data (non-empty)
    assert!(!output.stdout.is_empty());
});

// Test decompression with -c (stdout) flag
add_test!(decompress_to_stdout, async {
    const FILE_NAME: &str = "stdout_decompress_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress to stdout
    let output = fixture
        .run_cargo("xz", &["-d", "-c", &compressed_path])
        .await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);

    // Compressed file should still exist
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});

// Test compression with repetitive data
add_test!(compress_repetitive_data, async {
    const FILE_NAME: &str = "repetitive.txt";

    let data = REPETITIVE_DATA.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test compression with binary data
add_test!(compress_binary_data, async {
    const FILE_NAME: &str = "binary.bin";

    let mut fixture = Fixture::with_file(FILE_NAME, BINARY_DATA);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[BINARY_DATA]);
});

// Test compression with -t (test) flag
add_test!(test_integrity, async {
    const FILE_NAME: &str = "test_integrity.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Test integrity
    let output = fixture.run_cargo("xz", &["-t", &compressed_path]).await;
    assert!(output.status.success());

    // Compressed file should still exist
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});
