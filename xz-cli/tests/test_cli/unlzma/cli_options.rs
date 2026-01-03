use crate::add_test;
use crate::common::{generate_random_data, Fixture, SAMPLE_TEXT};
use crate::MB;

// Test `--threads` / `-T` option (ignored for `.lzma`).
add_test!(threads_ignored, async {
    const FILE_NAME: &str = "threads.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    // Create .lzma file.
    let output = fixture.run_cargo("lzma", &["-k", &file_path]).await;
    assert!(output.status.success(), "lzma failed: {}", output.stderr);

    fixture.remove_file(FILE_NAME);

    // `--threads` is accepted for CLI compatibility but ignored for `.lzma`.
    let output = fixture
        .run_cargo("unlzma", &["-T", "4", "-k", &lzma_path])
        .await;
    assert!(output.status.success(), "unlzma failed: {}", output.stderr);
    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test `--memory` / `--memlimit` / `-M` option.
add_test!(memlimit_too_small_is_reported, async {
    const FILE_NAME: &str = "memlimit.txt";

    // Large enough to ensure decompression needs more than a tiny limit.
    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    // Use explicit lzma1 dict to ensure predictable memory requirements.
    let out = fixture
        .run_cargo(
            "xz",
            &[
                "--format=lzma",
                "--lzma1",
                "dict=16MiB,lc=3,lp=0,pb=2,mode=normal,mf=bt4,nice=128,depth=0",
                "-k",
                &file_path,
            ],
        )
        .await;
    assert!(out.status.success(), "xz failed: {}", out.stderr);

    fixture.remove_file(FILE_NAME);

    // Decompress with a very small limit.
    let out = fixture.run_cargo("unlzma", &["-M", "1K", &lzma_path]).await;
    assert!(!out.status.success(), "unlzma unexpectedly succeeded");
    assert!(
        out.stderr.contains("Memory usage limit reached"),
        "unexpected stderr: {}",
        out.stderr
    );
});

// Test `--suffix` option.
add_test!(custom_suffix_option, async {
    const FILE_NAME: &str = "suffix.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // Create .foo using lzma.
    let out = fixture
        .run_cargo("lzma", &["--suffix=.foo", "-k", &file_path])
        .await;
    assert!(out.status.success(), "lzma failed: {}", out.stderr);
    assert!(fixture.file_exists("suffix.txt.foo"));

    fixture.remove_file(FILE_NAME);

    let foo_path = format!("{}.foo", fixture.path(FILE_NAME));
    let out = fixture
        .run_cargo("unlzma", &["--suffix=.foo", &foo_path])
        .await;
    assert!(out.status.success(), "unlzma failed: {}", out.stderr);
    fixture.assert_files(&[FILE_NAME], &[data]);
});

// Test `--suffix` option without leading dot.
add_test!(custom_suffix_without_dot, async {
    const FILE_NAME: &str = "suffix_no_dot.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // Create `.foo` using lzma without leading dot in suffix.
    let out = fixture
        .run_cargo("lzma", &["--suffix=foo", "-k", &file_path])
        .await;
    assert!(out.status.success(), "lzma failed: {}", out.stderr);
    assert!(fixture.file_exists("suffix_no_dot.txt.foo"));

    fixture.remove_file(FILE_NAME);

    let foo_path = format!("{}.foo", fixture.path(FILE_NAME));
    let out = fixture
        .run_cargo("unlzma", &["--suffix=foo", &foo_path])
        .await;
    assert!(out.status.success(), "unlzma failed: {}", out.stderr);
    fixture.assert_files(&[FILE_NAME], &[data]);
});
