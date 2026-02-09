use crate::add_test;
use crate::common::Fixture;

// Test that a missing input file yields exit code 2.
add_test!(missing_operand_yields_exit_2, async {
    const FILE: &str = "present.txt";
    let contents = b"hello\n";

    let mut fixture = Fixture::with_file(FILE, contents);

    let out = fixture.run_cargo("xz", &[&fixture.path(FILE)]).await;
    assert!(out.status.success());

    let present_xz = fixture.compressed_path(FILE);
    let missing = fixture.path("missing.xz");
    let out = fixture.run_cargo("xzcmp", &[&present_xz, &missing]).await;
    assert!(out.status.code() == Some(2));
});

// Test that a single operand with an unknown suffix yields exit code 2.
add_test!(single_operand_unknown_suffix_is_error, async {
    const FILE: &str = "I want to vacation.zzz";
    let contents = b"data\n";

    let mut fixture = Fixture::with_file(FILE, contents);

    let out = fixture.run_cargo("xzcmp", &[&fixture.path(FILE)]).await;
    assert!(out.status.code() == Some(2));
});
