use crate::add_test;
use crate::common::{generate_random_data, Fixture, SAMPLE_TEXT};
use crate::KB;

// Test basic xzdec functionality
add_test!(basic_decompress, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with xzdec to stdout
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);

    // Compressed file should still exist (xzdec doesn't remove it)
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});

// Test xzdec preserves original compressed file
add_test!(preserves_compressed, async {
    const FILE_NAME: &str = "preserve.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Use xzdec
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());

    // Compressed file must still exist
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});

// Test xzdec with small file
add_test!(small_file, async {
    const FILE_NAME: &str = "small.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with xzdec
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzdec with binary data
add_test!(binary_data, async {
    const FILE_NAME: &str = "binary.bin";

    let mut binary_data = Vec::new();
    for i in 0..256 {
        binary_data.push(i as u8);
    }

    let mut fixture = Fixture::with_file(FILE_NAME, &binary_data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with xzdec
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == binary_data);
});

// Test xzdec with empty file
add_test!(empty_file, async {
    const FILE_NAME: &str = "empty.txt";

    let mut fixture = Fixture::with_file(FILE_NAME, b"");

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with xzdec
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
});

// Test xzdec only outputs to stdout
add_test!(stdout_only, async {
    const FILE_NAME: &str = "stdout_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzdec should only output to stdout
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);

    // Original file should not be recreated
    assert!(!fixture.file_exists(FILE_NAME));
});

// Test xzdec is simpler than xz -dc (decompression-only utility)
add_test!(decompression_only, async {
    const FILE_NAME: &str = "decompress_only.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzdec should decompress
    let xzdec_output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(xzdec_output.status.success());

    // Compare with xz -dc
    let xz_output = fixture
        .run_cargo("xz", &["-d", "-c", &compressed_path])
        .await;
    assert!(xz_output.status.success());

    // Both should produce same output
    assert!(xzdec_output.stdout == xz_output.stdout);
});

// Test xzdec with file path argument
add_test!(with_file_argument, async {
    const FILE_NAME: &str = "file_arg.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzdec with file argument
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});
