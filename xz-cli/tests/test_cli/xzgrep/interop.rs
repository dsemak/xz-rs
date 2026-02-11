use crate::add_test;
use crate::common::Fixture;

// Test that system `xzgrep` matches our output and exit code when available.
add_test!(system_xzgrep_matches_output_when_available, async {
    const FILE: &str = "file.txt";
    let contents = b"alpha\nbeta\ngamma\n";

    let mut fixture = Fixture::with_file(FILE, contents);

    let out = fixture.run_cargo("xz", &[&fixture.path(FILE)]).await;
    assert!(out.status.success());

    let file_xz = fixture.compressed_path(FILE);

    let ours = fixture.run_cargo("xzgrep", &["beta", &file_xz]).await;
    if let Some(sys) = fixture.run_system("xzgrep", &["beta", &file_xz]).await {
        assert!(ours.status.code() == sys.status.code());
        assert!(ours.stdout_raw == sys.stdout_raw);
    }
});
