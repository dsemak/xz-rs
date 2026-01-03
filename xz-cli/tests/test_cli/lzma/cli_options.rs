use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

add_test!(lzma1_options_roundtrip, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    let opts = "dict=1MiB,lc=4,lp=0,pb=2,mode=fast,mf=hc4,nice=64,depth=128";
    let output = fixture
        .run_cargo("xz", &["--format=lzma", "--lzma1", opts, "-k", &file_path])
        .await;
    assert!(output.status.success(), "xz failed: {}", output.stderr);
    assert!(fixture.file_exists("test.txt.lzma"));

    fixture.remove_file(FILE_NAME);

    let output = fixture.run_cargo("unlzma", &[&lzma_path]).await;
    assert!(output.status.success(), "unlzma failed: {}", output.stderr);
    fixture.assert_files(&[FILE_NAME], &[data]);
});

add_test!(lzma1_options_invalid_rejected, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    let output = fixture
        .run_cargo("xz", &["--format=lzma", "--lzma1=lc=9", "-k", &file_path])
        .await;
    assert!(!output.status.success());
    assert!(
        output.stderr.contains("lzma1") || output.stderr.contains("lc"),
        "unexpected stderr: {}",
        output.stderr
    );
});
