use crate::add_test;
use crate::common::Fixture;

// Test that a missing compressed file yields exit code 2.
add_test!(missing_compressed_file_yields_exit_2, async {
    let mut fixture = Fixture::with_file("file.txt", b"hello\n");

    let missing = fixture.path("b.txt.xz");
    let out = fixture
        .run_cargo_with_env("xzless", &[&missing], &[("PAGER", "cat")])
        .await;

    assert!(out.status.code() == Some(2));
});

// Test that `-` cannot be combined with other files.
add_test!(dash_with_other_files_is_error, async {
    let mut fixture = Fixture::with_file("file.txt", b"foo\n");
    let file = fixture.path("file.txt");

    let out = fixture
        .run_cargo_with_env("xzless", &["-", &file], &[("PAGER", "cat")])
        .await;
    assert!(out.status.code() == Some(2));
});
