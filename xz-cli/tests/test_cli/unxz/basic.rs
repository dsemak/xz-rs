use crate::add_test;
use crate::common::{generate_random_data, Fixture, SAMPLE_TEXT};
use crate::MB;

// Test basic unxz decompression
add_test!(basic_decompress, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with xz first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with unxz
    let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test unxz with -k (keep) flag
add_test!(keep_compressed, async {
    const FILE_NAME: &str = "keep_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with keep
    let output = fixture.run_cargo("unxz", &["-k", &compressed_path]).await;
    assert!(output.status.success());

    // Both files should exist
    assert!(fixture.file_exists(FILE_NAME));
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});

// Test unxz with -c (stdout) flag
add_test!(decompress_to_stdout, async {
    const FILE_NAME: &str = "stdout_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress to stdout
    let output = fixture.run_cargo("unxz", &["-c", &compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);

    // Compressed file should still exist
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});

// Test unxz with -f (force) flag
add_test!(force_overwrite, async {
    const FILE_NAME: &str = "force_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress
    let output = fixture.run_cargo("unxz", &["-k", &compressed_path]).await;
    assert!(output.status.success());

    // Try to decompress again with force
    let output = fixture.run_cargo("unxz", &["-f", &compressed_path]).await;
    assert!(output.status.success());
});

// Test unxz with large file
add_test!(large_file, async {
    const FILE_NAME: &str = "large.txt";

    let data = generate_random_data(MB);
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

// Test unxz with multiple files
add_test!(multiple_files, async {
    const FILE_1: &str = "file1.txt";
    const FILE_2: &str = "file2.txt";
    const FILE_3: &str = "file3.txt";

    let data1 = SAMPLE_TEXT.as_bytes();
    let data2 = b"Second file content";
    let data3 = b"Third file content";

    let mut fixture = Fixture::with_files(&[FILE_1, FILE_2, FILE_3], &[data1, data2, data3]);

    let file_path_1 = fixture.path(FILE_1);
    let file_path_2 = fixture.path(FILE_2);
    let file_path_3 = fixture.path(FILE_3);

    // Compress all files
    let output = fixture
        .run_cargo("xz", &[&file_path_1, &file_path_2, &file_path_3])
        .await;
    assert!(output.status.success());

    let compressed_path_1 = fixture.compressed_path(FILE_1);
    let compressed_path_2 = fixture.compressed_path(FILE_2);
    let compressed_path_3 = fixture.compressed_path(FILE_3);

    // Decompress all with unxz
    let output = fixture
        .run_cargo(
            "unxz",
            &[&compressed_path_1, &compressed_path_2, &compressed_path_3],
        )
        .await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_1, FILE_2, FILE_3], &[data1, data2, data3]);
});

// Test unxz with -t (test) flag
add_test!(test_integrity, async {
    const FILE_NAME: &str = "test_integrity.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Test integrity with unxz
    let output = fixture.run_cargo("unxz", &["-t", &compressed_path]).await;
    assert!(output.status.success());

    // Compressed file should still exist
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});
