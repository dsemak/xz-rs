use crate::add_test;
use crate::common::Fixture;

// Test that `--help` exits successfully and prints usage text.
add_test!(help_option_prints_usage, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"pupupu");

    let out = fixture
        .run_cargo_with_env("xzless", &["--help"], &[("PAGER", "cat")])
        .await;
    assert!(out.status.success());
    assert!(out.stdout.contains("Usage:"));
});

// Test that `--version` exits successfully.
add_test!(version_option_succeeds, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"yayayay");

    let out = fixture
        .run_cargo_with_env("xzless", &["--version"], &[("PAGER", "cat")])
        .await;
    assert!(out.status.success());
});
