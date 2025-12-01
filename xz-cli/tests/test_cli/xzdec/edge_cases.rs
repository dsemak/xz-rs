use crate::add_test;
use crate::common::{generate_random_data, Fixture};
use crate::MB;

// Test xzdec with corrupted file
add_test!(corrupted_file, async {
    const FILE_NAME: &str = "corrupted.xz";

    let corrupted_data = b"This is not a valid xz file";
    let mut fixture = Fixture::with_file(FILE_NAME, corrupted_data);

    let file_path = fixture.path(FILE_NAME);

    // Should fail gracefully
    let output = fixture.run_cargo("xzdec", &[&file_path]).await;
    assert!(!output.status.success());
});

// Test xzdec with non-existent file
add_test!(non_existent_file, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    let non_existent = fixture.path("non_existent.xz");

    let output = fixture.run_cargo("xzdec", &[&non_existent]).await;
    assert!(!output.status.success());
});

// Test xzdec with large file
add_test!(large_file, async {
    const FILE_NAME: &str = "large.bin";

    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzdec should handle large files
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzdec with empty file
add_test!(empty_file, async {
    const FILE_NAME: &str = "empty.txt";

    let mut fixture = Fixture::with_file(FILE_NAME, b"");

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzdec should handle empty files
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
});

// Test xzdec with one byte file
add_test!(one_byte_file, async {
    const FILE_NAME: &str = "one.txt";

    let data = b"x";
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzdec
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzdec with all null bytes
add_test!(null_bytes_file, async {
    const FILE_NAME: &str = "nulls.bin";

    let data = vec![0u8; 10000];
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzdec
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzdec with truncated compressed file
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

    // xzdec should fail
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(!output.status.success());
});

// Test xzdec with uncompressed file having .xz extension
add_test!(uncompressed_with_xz_extension, async {
    const FILE_NAME: &str = "fake.xz";

    let data = b"This is not compressed";
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // xzdec should fail
    let output = fixture.run_cargo("xzdec", &[&file_path]).await;
    assert!(!output.status.success());
});

// Test xzdec with binary file containing all byte values
add_test!(all_byte_values, async {
    const FILE_NAME: &str = "all_bytes.bin";

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

    // xzdec
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == binary_data);
});

// Test xzdec with file having partial header
add_test!(partial_header, async {
    const FILE_NAME: &str = "partial.xz";

    // Create a file with only partial xz header
    let partial_header = b"\xFD\x37\x7A\x58";
    let mut fixture = Fixture::with_file(FILE_NAME, partial_header);

    let file_path = fixture.path(FILE_NAME);

    // xzdec should fail
    let output = fixture.run_cargo("xzdec", &[&file_path]).await;
    assert!(!output.status.success());
});

// Test xzdec with wrong file extension
add_test!(wrong_extension, async {
    const FILE_NAME: &str = "wrong.txt";

    let data = b"Some uncompressed data";
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // Try to decompress a non-compressed file
    let output = fixture.run_cargo("xzdec", &[&file_path]).await;
    assert!(!output.status.success());
});

// Test xzdec with random incompressible data
add_test!(incompressible_data, async {
    const FILE_NAME: &str = "random.dat";

    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress random data
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzdec should still decompress correctly
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});
