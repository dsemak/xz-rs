use std::fs::OpenOptions;
use std::io::Write;

use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

// Test that lzcat fails with trailing garbage by default and succeeds with `--single-stream`.
add_test!(trailing_garbage, async {
    const FILE_NAME: &str = "garbage.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    // Produce a valid .lzma file first.
    let out = fixture
        .run_cargo("lzma", &["--lzma1", "preset=6", "-k", &file_path])
        .await;
    assert!(out.status.success(), "lzma failed: {}", out.stderr);

    // Append trailing garbage.
    let mut f = OpenOptions::new()
        .append(true)
        .open(&lzma_path)
        .expect("open .lzma for append");
    f.write_all(b"TRAILING_GARBAGE").expect("append garbage");
    drop(f);

    // Strict by default: should fail.
    let out = fixture.run_cargo("lzcat", &[&lzma_path]).await;
    assert!(!out.status.success(), "lzcat unexpectedly succeeded");

    // Tolerant with --single-stream: should ignore the tail and succeed.
    let out = fixture
        .run_cargo("lzcat", &["--single-stream", &lzma_path])
        .await;
    assert!(
        out.status.success(),
        "lzcat --single-stream failed: {}",
        out.stderr
    );
    assert!(out.stdout_raw == data);
});
