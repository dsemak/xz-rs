use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

add_test!(basic_compress_decompress_roundtrip, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    // Compress to .lzma
    let output = fixture.run_cargo("lzma", &["-k", &file_path]).await;
    assert!(output.status.success(), "lzma failed: {}", output.stderr);
    assert!(fixture.file_exists("test.txt.lzma"));

    fixture.remove_file(FILE_NAME);

    // Decompress back
    let output = fixture.run_cargo("unlzma", &[&lzma_path]).await;
    assert!(output.status.success(), "unlzma failed: {}", output.stderr);
    fixture.assert_files(&[FILE_NAME], &[data]);
});

add_test!(lzcat_outputs_plaintext_to_stdout, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    // Create .lzma file
    let output = fixture.run_cargo("lzma", &["-k", &file_path]).await;
    assert!(output.status.success(), "lzma failed: {}", output.stderr);

    // lzcat should write decompressed bytes to stdout
    let output = fixture.run_cargo("lzcat", &[&lzma_path]).await;
    assert!(output.status.success(), "lzcat failed: {}", output.stderr);
    assert!(output.stdout_raw == data);
});


