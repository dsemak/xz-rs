use crate::add_test;
use crate::common::Fixture;

// Missing pattern should yield exit code 2.
add_test!(missing_pattern_yields_exit_2, async {
    let mut fixture = Fixture::with_file("file.txt", b"hello\n");

    let out = fixture
        .run_cargo("xzgrep", &[&fixture.path("file.txt")])
        .await;
    assert!(out.status.code() == Some(2));
});

// Missing compressed file should yield exit code 2.
add_test!(missing_compressed_file_yields_exit_2, async {
    let mut fixture = Fixture::with_file("present.txt", b"hello\n");

    let out = fixture
        .run_cargo("xz", &[&fixture.path("present.txt")])
        .await;
    assert!(out.status.success());

    let missing = fixture.path("missing.txt.xz");
    let out = fixture.run_cargo("xzgrep", &["hello", &missing]).await;
    assert!(out.status.code() == Some(2));
});

// `-` cannot be combined with other files.
add_test!(dash_with_other_files_is_error, async {
    let mut fixture = Fixture::with_file("a.txt", b"foo\n");

    let out = fixture.run_cargo("xz", &[&fixture.path("a.txt")]).await;
    assert!(out.status.success());

    let a_xz = fixture.compressed_path("a.txt");
    let out = fixture.run_cargo("xzgrep", &["foo", "-", &a_xz]).await;
    assert!(out.status.code() == Some(2));
});
