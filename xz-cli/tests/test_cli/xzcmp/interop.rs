use crate::add_test;
use crate::common::Fixture;

// Test that system `xzcmp` matches our exit codes when available.
add_test!(system_xzcmp_matches_exit_codes_when_available, async {
    const A: &str = "a.txt";
    const B: &str = "b.txt";
    const C: &str = "c.txt";

    let a = b"line 1\nline 2\n";
    let b = b"line 1\nline 2\n";
    let c = b"line 1\nDIFF\n";

    let mut fixture = Fixture::with_files(&[A, B, C], &[a, b, c]);

    let out = fixture.run_cargo("xz", &[&fixture.path(A)]).await;
    assert!(out.status.success());
    let out = fixture.run_cargo("xz", &[&fixture.path(B)]).await;
    assert!(out.status.success());
    let out = fixture.run_cargo("xz", &[&fixture.path(C)]).await;
    assert!(out.status.success());

    let a_xz = fixture.compressed_path(A);
    let b_xz = fixture.compressed_path(B);
    let c_xz = fixture.compressed_path(C);

    // Equal.
    let ours = fixture.run_cargo("xzcmp", &[&a_xz, &b_xz]).await;
    if let Some(sys) = fixture.run_system("xzcmp", &[&a_xz, &b_xz]).await {
        assert!(ours.status.code() == sys.status.code());
    }

    // Different.
    let ours = fixture.run_cargo("xzcmp", &[&a_xz, &c_xz]).await;
    if let Some(sys) = fixture.run_system("xzcmp", &[&a_xz, &c_xz]).await {
        assert!(ours.status.code() == sys.status.code());
    }
});
