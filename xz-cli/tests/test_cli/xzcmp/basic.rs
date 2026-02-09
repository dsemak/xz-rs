use crate::add_test;
use crate::common::Fixture;

// Test xzcmp exit codes for equal and different contents.
add_test!(exit_codes_equal_and_different, async {
    const SAME_1: &str = "same1.txt";
    const SAME_2: &str = "same2.txt";
    const DIFF: &str = "diff.txt";

    let same = b"same content\nsecond line\n";
    let different = b"same content\nDIFFERENT\n";

    let mut fixture = Fixture::with_files(&[SAME_1, SAME_2, DIFF], &[same, same, different]);

    // Create .xz files on disk.
    let out = fixture.run_cargo("xz", &[&fixture.path(SAME_1)]).await;
    assert!(out.status.success());
    let out = fixture.run_cargo("xz", &[&fixture.path(SAME_2)]).await;
    assert!(out.status.success());
    let out = fixture.run_cargo("xz", &[&fixture.path(DIFF)]).await;
    assert!(out.status.success());

    let same1_xz = fixture.compressed_path(SAME_1);
    let same2_xz = fixture.compressed_path(SAME_2);
    let diff_xz = fixture.compressed_path(DIFF);

    let out = fixture.run_cargo("xzcmp", &[&same1_xz, &same2_xz]).await;
    assert!(out.status.success());

    let out = fixture.run_cargo("xzcmp", &[&same1_xz, &diff_xz]).await;
    assert!(out.status.code() == Some(1));
});

// Test xzcmp with a single operand.
//
// This tests that xzcmp correctly compares the compressed file to the original file when
// only a single operand is provided.
add_test!(single_operand_comparison, async {
    const FILE: &str = "one.txt";
    let contents = b"hello\n";

    let mut fixture = Fixture::with_file(FILE, contents);

    // Keep the original so the inferred second operand exists.
    let file_path = fixture.path(FILE);
    let out = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(out.status.success());

    let file_xz = fixture.compressed_path(FILE);
    let out = fixture.run_cargo("xzcmp", &[&file_xz]).await;
    assert!(out.status.success());
});
