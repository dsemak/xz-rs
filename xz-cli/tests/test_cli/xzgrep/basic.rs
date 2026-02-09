use crate::add_test;
use crate::common::Fixture;

// Basic match and non-match exit codes.
add_test!(exit_codes_match_and_no_match, async {
    const FILE: &str = "file.txt";
    let contents = b"hello\nworld\n";

    let mut fixture = Fixture::with_file(FILE, contents);

    // Create .xz file on disk.
    let out = fixture.run_cargo("xz", &[&fixture.path(FILE)]).await;
    assert!(out.status.success());

    let file_xz = fixture.compressed_path(FILE);

    let out = fixture.run_cargo("xzgrep", &["hello", &file_xz]).await;
    assert!(out.status.success());
    assert!(out.stdout.contains("hello"));

    let out = fixture.run_cargo("xzgrep", &["nomatch", &file_xz]).await;
    assert!(out.status.code() == Some(1));
});

// When multiple files are specified, grep should prefix matching lines with the filename.
add_test!(multiple_files_prefix_filenames, async {
    const A: &str = "a.txt";
    const B: &str = "b.txt";

    let a = b"foo\n";
    let b = b"bar\nfoo\n";

    let mut fixture = Fixture::with_files(&[A, B], &[a, b]);

    let out = fixture.run_cargo("xz", &[&fixture.path(A)]).await;
    assert!(out.status.success());
    let out = fixture.run_cargo("xz", &[&fixture.path(B)]).await;
    assert!(out.status.success());

    let a_xz = fixture.compressed_path(A);
    let b_xz = fixture.compressed_path(B);

    let out = fixture.run_cargo("xzgrep", &["foo", &a_xz, &b_xz]).await;
    assert!(out.status.success());

    // Full paths are used as labels; ensure both files appear.
    assert!(out.stdout.contains(&format!("{a_xz}:foo")));
    assert!(out.stdout.contains(&format!("{b_xz}:foo")));
});
