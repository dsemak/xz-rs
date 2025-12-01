use crate::add_test;
use crate::common::{generate_random_data, Fixture};
use crate::MB;

// Test unxz with empty compressed file
add_test!(empty_file, async {
    const FILE_NAME: &str = "empty.txt";

    let mut fixture = Fixture::with_file(FILE_NAME, b"");

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress empty file
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with unxz
    let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[]);
});

// Test unxz with large file
add_test!(large_file, async {
    const FILE_NAME: &str = "large.bin";

    let data = generate_random_data(10 * MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with unxz
    let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[&data]);
});

// Test unxz with corrupted file
add_test!(corrupted_file, async {
    const FILE_NAME: &str = "corrupted.xz";

    let corrupted_data = b"This is not a valid xz file";
    let mut fixture = Fixture::with_file(FILE_NAME, corrupted_data);

    let file_path = fixture.path(FILE_NAME);

    // Should fail gracefully
    let output = fixture.run_cargo("unxz", &["-t", &file_path]).await;
    assert!(!output.status.success());

    let output = fixture.run_cargo("unxz", &["-f", &file_path]).await;
    assert!(!output.status.success());
});

// Test unxz with non-existent file
add_test!(non_existent_file, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    let non_existent = fixture.path("non_existent.xz");

    let output = fixture.run_cargo("unxz", &[&non_existent]).await;
    assert!(!output.status.success());
});

// Test unxz with file having wrong extension
add_test!(wrong_extension, async {
    const FILE_NAME: &str = "wrong.txt";

    let data = b"Some uncompressed data";
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // Try to decompress a non-compressed file
    let output = fixture.run_cargo("unxz", &["-f", &file_path]).await;
    assert!(!output.status.success());
});

// Test unxz with binary data
add_test!(binary_data, async {
    const FILE_NAME: &str = "binary.bin";

    // Create binary data with all byte values
    let mut binary_data = Vec::new();
    for i in 0..256 {
        binary_data.push(i as u8);
    }
    binary_data.extend_from_slice(&[0x00, 0xFF, 0xAA, 0x55].repeat(500));

    let mut fixture = Fixture::with_file(FILE_NAME, &binary_data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with unxz
    let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[&binary_data]);
});

// Test unxz with one byte file
add_test!(one_byte_file, async {
    const FILE_NAME: &str = "one_byte.txt";

    let data = b"x";
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with unxz
    let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test unxz with file containing only null bytes
add_test!(null_bytes_file, async {
    const FILE_NAME: &str = "nulls.bin";

    let data = vec![0u8; 10000];
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with unxz
    let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[&data]);
});

// Test unxz when output file already exists
add_test!(output_file_exists, async {
    const FILE_NAME: &str = "exists.txt";

    let data = b"original data";
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    // Try to decompress (output file already exists)
    // Should fail without -f
    let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
    assert!(!output.status.success());

    // With -f should succeed
    let output = fixture
        .run_cargo("unxz", &["-f", "-k", &compressed_path])
        .await;
    assert!(output.status.success());
});

// Test unxz with incomplete/truncated file
add_test!(truncated_file, async {
    const FILE_NAME: &str = "complete.txt";

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

    // Should fail
    let output = fixture.run_cargo("unxz", &["-f", &compressed_path]).await;
    assert!(!output.status.success());
});
