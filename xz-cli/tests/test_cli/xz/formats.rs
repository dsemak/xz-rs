use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

// Test .xz format compatibility
add_test!(xz_format, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress to .xz format
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    // Decompress .xz file
    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test file without extension
add_test!(no_extension, async {
    const FILE_NAME: &str = "README";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress file without extension
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    // Decompress
    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test file with multiple dots
add_test!(multiple_dots, async {
    const FILE_NAME: &str = "archive.tar.backup.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress file with multiple dots
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    // Decompress
    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test different file types
add_test!(various_extensions, async {
    const FILES: [&str; 5] = [
        "document.txt",
        "script.sh",
        "data.json",
        "archive.tar",
        "image.bmp",
    ];

    let data = SAMPLE_TEXT.as_bytes();

    for file_name in FILES {
        let mut fixture = Fixture::with_file(file_name, data);

        let file_path = fixture.path(file_name);
        let compressed_path = fixture.compressed_path(file_name);

        // Compress
        let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
        assert!(output.status.success(), "Failed for {file_name}");

        fixture.remove_file(file_name);

        // Decompress
        let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
        assert!(output.status.success(), "Failed for {file_name}");

        fixture.assert_files(&[file_name], &[data]);
    }
});

// Test file with leading dots (hidden files)
add_test!(hidden_file, async {
    const FILE_NAME: &str = ".hidden";

    let data = SAMPLE_TEXT.as_bytes();
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

// Test file with special characters in name
add_test!(special_chars_in_name, async {
    const FILE_NAME: &str = "file-with_special.chars.txt";

    let data = SAMPLE_TEXT.as_bytes();
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

// Test .tar.xz handling (common combination)
add_test!(tar_xz_format, async {
    const FILE_NAME: &str = "archive.tar";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress .tar to .tar.xz
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());
    assert!(fixture.file_exists("archive.tar.xz"));

    fixture.remove_file(FILE_NAME);

    // Decompress .tar.xz back to .tar
    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test file already with .xz extension
add_test!(already_xz_extension, async {
    const FILE_NAME: &str = "file.xz";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // Compress file that already has .xz extension
    let output = fixture.run_cargo("xz", &["-f", "-k", &file_path]).await;
    assert!(output.status.success());

    // Should create .xz.xz
    assert!(fixture.file_exists("file.xz.xz"));
});

// Test Unicode filename
add_test!(unicode_filename, async {
    const FILE_NAME: &str = "тест.txt";

    let data = SAMPLE_TEXT.as_bytes();
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
