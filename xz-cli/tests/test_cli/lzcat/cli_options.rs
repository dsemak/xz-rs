use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

// Test that `--threads`/`-T` is accepted for compatibility but ignored for `.lzma` files.
add_test!(threads_ignored, async {
    const FILE_NAME: &str = "threads.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    // Create .lzma file.
    let output = fixture.run_cargo("lzma", &["-k", &file_path]).await;
    assert!(output.status.success(), "lzma failed: {}", output.stderr);

    // `--threads` is accepted for CLI compatibility but ignored for `.lzma`.
    let output = fixture.run_cargo("lzcat", &["-T", "4", &lzma_path]).await;
    assert!(output.status.success(), "lzcat failed: {}", output.stderr);
    assert!(output.stdout_raw == data);
});
