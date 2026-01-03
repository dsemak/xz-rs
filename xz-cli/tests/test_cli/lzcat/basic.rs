use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

// Test basic lzcat functionality.
add_test!(basic_decompress, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    // Create .lzma file using our lzma.
    let output = fixture.run_cargo("lzma", &["-k", &file_path]).await;
    assert!(output.status.success(), "lzma failed: {}", output.stderr);

    // lzcat should write decompressed bytes to stdout.
    let output = fixture.run_cargo("lzcat", &[&lzma_path]).await;
    assert!(output.status.success(), "lzcat failed: {}", output.stderr);
    assert!(output.stdout_raw == data);
});
