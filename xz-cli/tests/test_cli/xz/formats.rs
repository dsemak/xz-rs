use std::fs;

use crate::add_test;
use crate::common::{BinaryType, Fixture, SAMPLE_TEXT};

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

// Test `xz --format=lzma` creates `.lzma` output and is decodable.
add_test!(lzma_format_via_xz, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    // Compress to .lzma using xz
    let output = fixture
        .run_cargo("xz", &["--format=lzma", "-k", &file_path])
        .await;
    assert!(output.status.success(), "xz failed: {}", output.stderr);
    assert!(fixture.file_exists("test.txt.lzma"));

    fixture.remove_file(FILE_NAME);

    // Decompress .lzma back using xz auto decoder
    let output = fixture.run_cargo("xz", &["-d", &lzma_path]).await;
    assert!(output.status.success(), "xz -d failed: {}", output.stderr);

    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test `.xz` mode accepts `--lzma2` together with a custom suffix.
add_test!(xz_format_with_lzma2_suffix_roundtrip, async {
    const FILE_NAME: &str = "lzma2_suffix.txt";
    const XZ_FILE: &str = "lzma2_suffix.txt.foo";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let xz_path = fixture.path(XZ_FILE);

    let output = fixture
        .run_cargo(
            "xz",
            &["-z", "-k", "--suffix=.foo", "--lzma2=preset=0", &file_path],
        )
        .await;
    assert!(output.status.success());
    assert!(fixture.file_exists(XZ_FILE));

    fixture.remove_file(FILE_NAME);

    let output = fixture
        .run_cargo("xz", &["-d", "--suffix=.foo", &xz_path])
        .await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
    assert!(!fixture.file_exists(XZ_FILE));
});

// Test raw mode accepts an explicit suffix in file mode for compression and decompression.
add_test!(raw_format_with_suffix_roundtrip, async {
    const FILE_NAME: &str = "raw_suffix.txt";
    const RAW_FILE: &str = "raw_suffix.txt.foo";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let raw_path = fixture.path(RAW_FILE);

    let output = fixture
        .run_cargo(
            "xz",
            &[
                "-z",
                "-k",
                "--format=raw",
                "--lzma1=preset=0",
                "--suffix=.foo",
                &file_path,
            ],
        )
        .await;
    assert!(output.status.success());
    assert!(fixture.file_exists(RAW_FILE));

    fixture.remove_file(FILE_NAME);

    let output = fixture
        .run_cargo(
            "xz",
            &[
                "-d",
                "--format=raw",
                "--lzma1=preset=0",
                "--suffix=.foo",
                &raw_path,
            ],
        )
        .await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[data]);
    assert!(!fixture.file_exists(RAW_FILE));
});

// Test raw mode still rejects file mode when no suffix is available for renaming.
add_test!(raw_format_without_suffix_rejected_in_file_mode, async {
    const FILE_NAME: &str = "raw_no_suffix.txt";
    const RAW_FILE: &str = "raw_no_suffix.txt.foo";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let raw_path = fixture.path(RAW_FILE);

    let output = fixture
        .run_cargo(
            "xz",
            &["-z", "-k", "--format=raw", "--lzma1=preset=0", &file_path],
        )
        .await;
    assert!(!output.status.success());
    assert!(!fixture.file_exists(RAW_FILE));

    let output = fixture
        .run_cargo(
            "xz",
            &[
                "-z",
                "-k",
                "--format=raw",
                "--lzma1=preset=0",
                "--suffix=.foo",
                &file_path,
            ],
        )
        .await;
    assert!(output.status.success());
    assert!(fixture.file_exists(RAW_FILE));

    fixture.remove_file(FILE_NAME);

    let output = fixture
        .run_cargo("xz", &["-d", "--format=raw", "--lzma1=preset=0", &raw_path])
        .await;
    assert!(!output.status.success());
    assert!(fixture.file_exists(RAW_FILE));
    assert!(!fixture.file_exists(FILE_NAME));
});

// Test raw mode detects implicit stdout when reading from stdin.
add_test!(raw_format_stdin_writes_to_stdout, async {
    let mut fixture = Fixture::with_file("stdin-anchor.txt", b"anchor");

    let output = fixture
        .run_with_stdin_raw(
            BinaryType::cargo("xz"),
            &["--format=raw", "--lzma1=preset=0"],
            b"foo",
        )
        .await;
    assert!(output.status.success());
    assert!(!output.stdout_raw.is_empty());
});

// Test raw mode rejects --files without a suffix because it cannot derive output names.
add_test!(raw_format_files_requires_suffix, async {
    const FILE_NAME: &str = "raw_files_input.txt";
    const LIST_FILE: &str = "raw_files_list.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let list_path = fixture.path(LIST_FILE);
    fs::write(&list_path, format!("{file_path}\n")).unwrap();

    let output = fixture
        .run_cargo(
            "xz",
            &["--format=raw", "--lzma1=preset=0", "--files", &list_path],
        )
        .await;
    assert!(!output.status.success());
    assert!(!fixture.file_exists("raw_files_input.txt.foo"));
});

// Test raw mode accepts --files when an explicit suffix is provided.
add_test!(raw_format_files_accepts_suffix, async {
    const FILE_NAME: &str = "raw_files_suffix_input.txt";
    const LIST_FILE: &str = "raw_files_suffix_list.txt";
    const RAW_FILE: &str = "raw_files_suffix_input.txt.foo";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let list_path = fixture.path(LIST_FILE);
    fs::write(&list_path, format!("{file_path}\n")).unwrap();

    let output = fixture
        .run_cargo(
            "xz",
            &[
                "-z",
                "-k",
                "--format=raw",
                "--lzma1=preset=0",
                "--suffix=.foo",
                "--files",
                &list_path,
            ],
        )
        .await;
    assert!(output.status.success());
    assert!(fixture.file_exists(RAW_FILE));
});

// Test raw mode rejects --files0 without a suffix because it cannot derive output names.
add_test!(raw_format_files0_requires_suffix, async {
    const FILE_NAME: &str = "raw_files0_input.txt";
    const LIST_FILE: &str = "raw_files0_list.bin";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let list_path = fixture.path(LIST_FILE);
    let mut list_bytes = Vec::new();
    list_bytes.extend_from_slice(file_path.as_bytes());
    list_bytes.push(0);
    fs::write(&list_path, list_bytes).unwrap();

    let output = fixture
        .run_cargo(
            "xz",
            &["--format=raw", "--lzma1=preset=0", "--files0", &list_path],
        )
        .await;
    assert!(!output.status.success());
    assert!(!fixture.file_exists("raw_files0_input.txt.foo"));
});

// Test raw mode accepts --files0 when an explicit suffix is provided.
add_test!(raw_format_files0_accepts_suffix, async {
    const FILE_NAME: &str = "raw_files0_suffix_input.txt";
    const LIST_FILE: &str = "raw_files0_suffix_list.bin";
    const RAW_FILE: &str = "raw_files0_suffix_input.txt.foo";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let list_path = fixture.path(LIST_FILE);
    let mut list_bytes = Vec::new();
    list_bytes.extend_from_slice(file_path.as_bytes());
    list_bytes.push(0);
    fs::write(&list_path, list_bytes).unwrap();

    let output = fixture
        .run_cargo(
            "xz",
            &[
                "-z",
                "-k",
                "--format=raw",
                "--lzma1=preset=0",
                "--suffix=.foo",
                "--files0",
                &list_path,
            ],
        )
        .await;
    assert!(output.status.success());
    assert!(fixture.file_exists(RAW_FILE));
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
