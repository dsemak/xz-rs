use crate::add_test;
use crate::common::Fixture;

// Test that xzmore shows decompressed content for .xz files.
add_test!(shows_decompressed_content, async {
    const FILE: &str = "file.txt";
    let contents = b"line one\nline two\n";

    let mut fixture = Fixture::with_file(FILE, contents);

    let out = fixture.run_cargo("xz", &["-k", &fixture.path(FILE)]).await;
    assert!(out.status.success());

    let file_xz = fixture.compressed_path(FILE);
    let out = fixture
        .run_cargo_with_env("xzmore", &[&file_xz], &[("PAGER", "cat")])
        .await;

    assert!(out.status.success());
    assert!(out.stdout_raw == contents);
});

// Test that xzmore forwards pager options to the selected pager.
add_test!(forwards_pager_options, async {
    const FILE: &str = "opts.txt";
    let contents = b"blabla\nblublu\n";

    let mut fixture = Fixture::with_file(FILE, contents);

    let out = fixture.run_cargo("xz", &["-k", &fixture.path(FILE)]).await;
    assert!(out.status.success());

    let file_xz = fixture.compressed_path(FILE);
    let out = fixture
        .run_cargo_with_env("xzmore", &["-n", &file_xz], &[("PAGER", "cat")])
        .await;

    assert!(out.status.success());
    assert!(out.stdout.contains("1\tblabla"));
    assert!(out.stdout.contains("2\tblublu"));
});
