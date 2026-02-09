use crate::add_test;
use crate::common::Fixture;

// Test that `--help` exits successfully and prints usage text.
add_test!(help_option_prints_usage, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    let out = fixture.run_cargo("xzcmp", &["--help"]).await;
    assert!(out.status.success());
    assert!(out.stdout.contains("Usage:"));
});

// Test that `--version` exits successfully.
add_test!(version_option_succeeds, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    let out = fixture.run_cargo("xzcmp", &["--version"]).await;
    assert!(out.status.success());
});
