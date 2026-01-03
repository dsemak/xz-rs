use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

// Test system xz (format=lzma) -> our unlzma.
add_test!(system_xz_lzma_to_our_unlzma, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    let Some(system_out) = fixture
        .run_system("xz", &["--format=lzma", "-k", &file_path])
        .await
    else {
        return;
    };
    assert!(
        system_out.status.success(),
        "system xz failed: {}",
        system_out.stderr
    );

    fixture.remove_file(FILE_NAME);

    let output = fixture.run_cargo("unlzma", &[&lzma_path]).await;
    assert!(output.status.success(), "unlzma failed: {}", output.stderr);
    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test our xz (format=lzma) -> system xz -d.
add_test!(our_xz_lzma_to_system_xz, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    let output = fixture
        .run_cargo("xz", &["--format=lzma", "-k", &file_path])
        .await;
    assert!(output.status.success(), "our xz failed: {}", output.stderr);

    fixture.remove_file(FILE_NAME);

    let Some(system_out) = fixture.run_system("xz", &["-d", &lzma_path]).await else {
        return;
    };
    assert!(
        system_out.status.success(),
        "system xz -d failed: {}",
        system_out.stderr
    );
});
