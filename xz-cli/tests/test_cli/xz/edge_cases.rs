use crate::add_test;
use crate::common::{generate_random_data, Fixture};
use crate::MB;

// Test empty file handling
add_test!(empty_file, async {
    const FILE_NAME: &str = "empty.txt";

    let mut fixture = Fixture::with_file(FILE_NAME, b"");

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress empty file
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    // Decompress and verify
    let output = fixture
        .run_cargo("xz", &["-d", "-f", &compressed_path])
        .await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[]);
});

// Test large file handling (10 MB)
add_test!(large_file, async {
    const FILE_NAME: &str = "large.txt";

    let data = generate_random_data(10 * MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress large file
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    // Decompress and verify
    let output = fixture
        .run_cargo("xz", &["-d", "-f", &compressed_path])
        .await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[&data]);
});

// Test corrupted file handling
add_test!(corrupted_file, async {
    const FILE_NAME: &str = "corrupted.xz";

    // Create a corrupted xz file
    let corrupted_data = b"This is not a valid xz file, just some random text";
    let mut fixture = Fixture::with_file(FILE_NAME, corrupted_data);

    let file_path = fixture.path(FILE_NAME);

    // Test should fail gracefully
    let output = fixture.run_cargo("xz", &["-t", &file_path]).await;
    assert!(!output.status.success());

    let output = fixture.run_cargo("xz", &["-d", "-f", &file_path]).await;
    assert!(!output.status.success());

    // Original corrupted file should still exist (with -f it stays)
    fixture.assert_files(&[FILE_NAME], &[corrupted_data]);
});

// Test binary file with all byte values
add_test!(binary_all_bytes, async {
    const FILE_NAME: &str = "all_bytes.bin";

    // Create binary data with all possible byte values
    let mut binary_data = Vec::new();
    for i in 0..256 {
        binary_data.push(i as u8);
    }
    // Repeat pattern multiple times
    for _ in 0..1000 {
        binary_data.extend_from_slice(&[0x00, 0xFF, 0xAA, 0x55]);
    }

    let mut fixture = Fixture::with_file(FILE_NAME, &binary_data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress binary file
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    // Decompress and verify
    let output = fixture
        .run_cargo("xz", &["-d", "-f", &compressed_path])
        .await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[&binary_data]);
});

// Test file that cannot be compressed well (random data)
add_test!(incompressible_data, async {
    const FILE_NAME: &str = "random.dat";

    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress random data (likely won't compress well)
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    // Decompress and verify
    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[&data]);
});

// Test very small file (1 byte)
add_test!(one_byte_file, async {
    const FILE_NAME: &str = "one_byte.txt";

    let data = b"a";
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

// Test file with null bytes
add_test!(null_bytes_file, async {
    const FILE_NAME: &str = "nulls.bin";

    let data = vec![0u8; 1000];
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[&data]);
});

// Test handling of already compressed file
add_test!(already_compressed, async {
    const FILE_NAME: &str = "already.txt";

    let data = b"test data";
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // First compression
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Try to compress again (should work with -f)
    let output = fixture
        .run_cargo("xz", &["-f", "-k", &compressed_path])
        .await;
    assert!(output.status.success());

    // Should create .xz.xz file
    assert!(fixture.file_exists(&format!("{}.xz.xz", FILE_NAME)));
});

// Test decompression of non-existent file
add_test!(non_existent_file, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    let non_existent = fixture.path("non_existent.xz");

    let output = fixture.run_cargo("xz", &["-d", &non_existent]).await;
    assert!(!output.status.success());
});
