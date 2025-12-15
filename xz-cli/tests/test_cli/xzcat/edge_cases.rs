use crate::add_test;
use crate::common::{generate_random_data, BinaryType, Fixture};
use crate::{KB, MB};

// Test xzcat with corrupted file
add_test!(corrupted_file, async {
    const FILE_NAME: &str = "corrupted.xz";

    let corrupted_data = b"This is not a valid xz file";
    let mut fixture = Fixture::with_file(FILE_NAME, corrupted_data);

    let file_path = fixture.path(FILE_NAME);

    // Should fail gracefully
    let output = fixture.run_cargo("xzcat", &[&file_path]).await;
    assert!(!output.status.success());
});

// Test xzcat with non-existent file
add_test!(non_existent_file, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    let non_existent = fixture.path("non_existent.xz");

    let output = fixture.run_cargo("xzcat", &[&non_existent]).await;
    assert!(!output.status.success());
});

// Test xzcat with large file
add_test!(large_file, async {
    const FILE_NAME: &str = "large.bin";

    let data = generate_random_data(10 * KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzcat should handle large files
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzcat with empty file
add_test!(empty_file, async {
    const FILE_NAME: &str = "empty.txt";

    let mut fixture = Fixture::with_file(FILE_NAME, b"");

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzcat should handle empty files
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
});

// Test xzcat with mix of valid and invalid files
add_test!(mixed_valid_invalid, async {
    const VALID_FILE: &str = "valid.txt";
    const INVALID_FILE: &str = "invalid.xz";

    let data = b"valid data";
    let invalid = b"not compressed";

    let mut fixture = Fixture::with_files(&[VALID_FILE, INVALID_FILE], &[data, invalid]);

    let file_path = fixture.path(VALID_FILE);
    let compressed_path = fixture.compressed_path(VALID_FILE);
    let invalid_path = fixture.path(INVALID_FILE);

    // Compress valid file
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Try xzcat with valid then invalid file
    let output = fixture
        .run_cargo("xzcat", &[&compressed_path, &invalid_path])
        .await;
    // Should fail due to invalid file
    assert!(!output.status.success());
});

// Test xzcat with one byte file
add_test!(one_byte_file, async {
    const FILE_NAME: &str = "one.txt";

    let data = b"x";
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzcat
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzcat with all null bytes
add_test!(null_bytes_file, async {
    const FILE_NAME: &str = "nulls.bin";

    let data = vec![0u8; 10000];
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzcat
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzcat with truncated compressed file
add_test!(truncated_file, async {
    const FILE_NAME: &str = "truncated.txt";

    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Truncate the compressed file
    let compressed_full_path = fixture.root_dir_path().join(&compressed_path);
    let compressed_data = std::fs::read(&compressed_full_path).unwrap();
    let truncated = &compressed_data[..compressed_data.len() / 2];
    std::fs::write(&compressed_full_path, truncated).unwrap();

    // xzcat should fail
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(!output.status.success());
});

// Test xzcat with uncompressed file having .xz extension
add_test!(uncompressed_with_xz_extension, async {
    const FILE_NAME: &str = "fake.xz";

    let data = b"This is not compressed";
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // xzcat should fail
    let output = fixture.run_cargo("xzcat", &[&file_path]).await;
    assert!(!output.status.success());
});

// Test xzcat with many small files
add_test!(many_small_files, async {
    const NUM_FILES: usize = 10;

    let mut file_names = Vec::new();
    let mut file_data = Vec::new();
    let mut compressed_paths = Vec::new();

    for i in 0..NUM_FILES {
        let name = format!("file{}.txt", i);
        file_names.push(name.clone());
        file_data.push(format!("Content {}", i).into_bytes());
    }

    let data_refs: Vec<&[u8]> = file_data.iter().map(|v| v.as_slice()).collect();
    let name_refs: Vec<&str> = file_names.iter().map(|s| s.as_str()).collect();

    let mut fixture = Fixture::with_files(&name_refs, &data_refs);

    // Compress all files
    for name in &file_names {
        let file_path = fixture.path(name);
        let output = fixture.run_cargo("xz", &[&file_path]).await;
        assert!(output.status.success());
        compressed_paths.push(fixture.compressed_path(name));
    }

    // xzcat all files
    let compressed_refs: Vec<&str> = compressed_paths.iter().map(|s| s.as_str()).collect();
    let output = fixture.run_cargo("xzcat", &compressed_refs).await;
    assert!(output.status.success());

    // Verify concatenated output
    let expected: Vec<u8> = file_data.into_iter().flatten().collect();
    assert!(output.stdout_raw == expected);
});

// Test `-` as stdin in the middle of the file list.
add_test!(dash_reads_stdin_in_middle, async {
    const FILE_1: &str = "file1.txt";
    const FILE_2: &str = "file2.txt";
    const STDIN_FILE: &str = "stdin.txt";

    let data_1 = b"file1 data";
    let data_2 = b"file2 data";
    let stdin_data = b"stdin data";

    let mut fixture = Fixture::with_files(
        &[FILE_1, FILE_2, STDIN_FILE],
        &[data_1, data_2, stdin_data],
    );

    // Prepare file inputs as .xz files on disk.
    let file_1_path = fixture.path(FILE_1);
    let file_2_path = fixture.path(FILE_2);
    let file_1_xz = fixture.compressed_path(FILE_1);
    let file_2_xz = fixture.compressed_path(FILE_2);

    let output = fixture.run_cargo("xz", &[&file_1_path]).await;
    assert!(output.status.success());
    let output = fixture.run_cargo("xz", &[&file_2_path]).await;
    assert!(output.status.success());

    // Prepare stdin as XZ-compressed bytes.
    let stdin_path = fixture.path(STDIN_FILE);
    let output = fixture.run_cargo("xz", &["-c", &stdin_path]).await;
    assert!(output.status.success());
    let stdin_compressed = output.stdout_raw;

    // xzcat should accept '-' as stdin in the file list.
    let output = fixture
        .run_with_stdin_raw(
            BinaryType::cargo("xzcat"),
            &[&file_1_xz, "-", &file_2_xz],
            &stdin_compressed,
        )
        .await;
    assert!(output.status.success());

    let expected: Vec<u8> = data_1
        .iter()
        .chain(stdin_data.iter())
        .chain(data_2.iter())
        .copied()
        .collect();
    assert!(output.stdout_raw == expected);
});
