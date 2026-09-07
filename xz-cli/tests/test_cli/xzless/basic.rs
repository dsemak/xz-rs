use crate::add_test;
use crate::common::{BinaryType, Fixture};

// Test that xzless shows decompressed content for .xz files.
add_test!(shows_decompressed_content, async {
    const FILE: &str = "file.txt";
    let contents = b"line one\nline two\n";

    let mut fixture = Fixture::with_file(FILE, contents);

    let out = fixture.run_cargo("xz", &["-k", &fixture.path(FILE)]).await;
    assert!(out.status.success());

    let file_xz = fixture.compressed_path(FILE);
    let out = fixture
        .run_cargo_with_env("xzless", &[&file_xz], &[("PAGER", "cat")])
        .await;

    assert!(out.status.success());
    assert!(out.stdout_raw == contents);
});

// Test that xzless forwards pager options to the selected pager.
add_test!(forwards_pager_options, async {
    const FILE: &str = "opts.txt";
    let contents = b"blabla\nblublu\n";

    let mut fixture = Fixture::with_file(FILE, contents);

    let out = fixture.run_cargo("xz", &["-k", &fixture.path(FILE)]).await;
    assert!(out.status.success());

    let file_xz = fixture.compressed_path(FILE);
    let out = fixture
        .run_cargo_with_env("xzless", &["-n", &file_xz], &[("PAGER", "cat")])
        .await;

    assert!(out.status.success());
    assert!(out.stdout.contains("1\tblabla"));
    assert!(out.stdout.contains("2\tblublu"));
});

// Compressed stdin must be decompressed before the pager sees it.
add_test!(stdin_decompresses_before_pager, async {
    let mut fixture = Fixture::with_file("stdin-anchor.txt", b"anchor");
    let contents = b"line one\nline two\n";

    let compressed = fixture
        .run_with_stdin_raw(BinaryType::cargo("xz"), &["-c"], contents)
        .await;
    assert!(compressed.status.success());

    let out = fixture
        .run_with_stdin_raw_env(
            BinaryType::cargo("xzless"),
            &[],
            &compressed.stdout_raw,
            &[("PAGER", "cat")],
        )
        .await;

    assert!(out.status.success());
    assert_eq!(out.stdout_raw.as_slice(), contents);
});
