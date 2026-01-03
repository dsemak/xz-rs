use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

// Test unxz with .xz extension
add_test!(xz_extension, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
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

// Test unxz with .tar.xz format
add_test!(tar_xz_format, async {
    const FILE_NAME: &str = "archive.tar";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());
    assert!(fixture.file_exists("archive.tar.xz"));

    // Decompress with unxz
    let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
    assert!(output.status.success());

    // Should restore to .tar
    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test unxz with files having multiple dots
add_test!(multiple_dots, async {
    const FILE_NAME: &str = "backup.2024.01.15.txt";

    let data = SAMPLE_TEXT.as_bytes();
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

// Test unxz with various file extensions
add_test!(various_extensions, async {
    const FILES: [&str; 6] = [
        "document.txt",
        "script.sh",
        "data.json",
        "archive.tar",
        "config.yaml",
        "log.log",
    ];

    let data = SAMPLE_TEXT.as_bytes();

    for file_name in FILES {
        let mut fixture = Fixture::with_file(file_name, data);

        let file_path = fixture.path(file_name);
        let compressed_path = fixture.compressed_path(file_name);

        // Compress
        let output = fixture.run_cargo("xz", &[&file_path]).await;
        assert!(
            output.status.success(),
            "Compression failed for {file_name}"
        );

        // Decompress with unxz
        let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
        assert!(
            output.status.success(),
            "Decompression failed for {file_name}"
        );

        fixture.assert_files(&[file_name], &[data]);
    }
});

// Test unxz with hidden files (dot files)
add_test!(hidden_files, async {
    const FILE_NAME: &str = ".hidden_config";

    let data = SAMPLE_TEXT.as_bytes();
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

// Test unxz with file without extension
add_test!(no_extension, async {
    const FILE_NAME: &str = "Makefile";

    let data = SAMPLE_TEXT.as_bytes();
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

// Test unxz with Unicode filenames
add_test!(unicode_filename, async {
    const FILE_NAME: &str = "файл.txt";

    let data = SAMPLE_TEXT.as_bytes();
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

// Test unxz with special characters in filename
add_test!(special_chars_filename, async {
    const FILE_NAME: &str = "file-with_special@chars.txt";

    let data = SAMPLE_TEXT.as_bytes();
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

// Test unxz with doubly compressed file (.xz.xz)
add_test!(double_compression, async {
    const FILE_NAME: &str = "double.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // First compression
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Second compression
    let output = fixture.run_cargo("xz", &["-f", &compressed_path]).await;
    assert!(output.status.success());
    assert!(fixture.file_exists(&format!("{FILE_NAME}.xz.xz")));

    // First decompression
    let double_compressed = fixture.path(&format!("{FILE_NAME}.xz.xz"));
    let output = fixture.run_cargo("unxz", &[&double_compressed]).await;
    assert!(output.status.success());
    assert!(fixture.file_exists(&format!("{FILE_NAME}.xz")));

    // Second decompression
    let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
});
