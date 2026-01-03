use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

// Test `--lzma1` option.
add_test!(lzma1_option, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    let opts = "dict=1MiB,lc=4,lp=0,pb=2,mode=fast,mf=hc4,nice=64,depth=128";
    let output = fixture
        .run_cargo("lzma", &["--lzma1", opts, "-k", &file_path])
        .await;
    assert!(output.status.success(), "lzma failed: {}", output.stderr);
    assert!(fixture.file_exists("test.txt.lzma"));

    fixture.remove_file(FILE_NAME);

    let output = fixture.run_cargo("unlzma", &[&lzma_path]).await;
    assert!(output.status.success(), "unlzma failed: {}", output.stderr);
    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test invalid `--lzma1` option.
add_test!(lzma1_options_invalid_rejected, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    let output = fixture
        .run_cargo("lzma", &["--lzma1=lc=9", "-k", &file_path])
        .await;
    assert!(!output.status.success());
    assert!(
        output.stderr.contains("lzma1") || output.stderr.contains("lc"),
        "unexpected stderr: {}",
        output.stderr
    );
});

// Test `--threads` / `-T` option (ignored for `.lzma`).
add_test!(threads_ignored, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // `--threads` is accepted for CLI compatibility but ignored for `.lzma`.
    let output = fixture
        .run_cargo("lzma", &["-T", "4", "-k", &file_path])
        .await;
    assert!(output.status.success(), "lzma failed: {}", output.stderr);
});

// Test `--suffix` option.
add_test!(custom_suffix_option, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    let out = fixture
        .run_cargo("lzma", &["--suffix=.foo", "-k", &file_path])
        .await;
    assert!(out.status.success(), "lzma failed: {}", out.stderr);
    assert!(fixture.file_exists("test.txt.foo"));
});

// Test `--suffix` option without leading dot.
add_test!(custom_suffix_without_dot, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    let out = fixture
        .run_cargo("lzma", &["--suffix=foo", "-k", &file_path])
        .await;
    assert!(out.status.success(), "lzma failed: {}", out.stderr);
    assert!(fixture.file_exists("test.txt.foo"));
});
