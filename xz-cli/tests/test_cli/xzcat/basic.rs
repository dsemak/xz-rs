use crate::add_test;
use crate::common::{generate_random_data, Fixture, SAMPLE_TEXT};
use crate::KB;

// Test basic xzcat functionality
add_test!(basic_decompress, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress to stdout with xzcat
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);

    // Compressed file should still exist
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});

// Test xzcat with multiple files
add_test!(multiple_files, async {
    const FILE_1: &str = "file1.txt";
    const FILE_2: &str = "file2.txt";
    const FILE_3: &str = "file3.txt";

    let data1 = generate_random_data(KB);
    let data2 = generate_random_data(KB);
    let data3 = generate_random_data(KB);

    let mut fixture = Fixture::with_files(&[FILE_1, FILE_2, FILE_3], &[&data1, &data2, &data3]);

    let file_path_1 = fixture.path(FILE_1);
    let file_path_2 = fixture.path(FILE_2);
    let file_path_3 = fixture.path(FILE_3);
    let compressed_path_1 = fixture.compressed_path(FILE_1);
    let compressed_path_2 = fixture.compressed_path(FILE_2);
    let compressed_path_3 = fixture.compressed_path(FILE_3);

    // Compress all files
    let output = fixture
        .run_cargo("xz", &["-k", &file_path_1, &file_path_2, &file_path_3])
        .await;
    assert!(output.status.success());

    // Use xzcat with multiple files
    let output = fixture
        .run_cargo(
            "xzcat",
            &[&compressed_path_1, &compressed_path_2, &compressed_path_3],
        )
        .await;
    assert!(output.status.success());

    // Output should be concatenation of all files
    let expected = data1
        .into_iter()
        .chain(data2.into_iter())
        .chain(data3.into_iter())
        .collect::<Vec<_>>();
    assert!(output.stdout_raw == expected);
});

// Test xzcat with single file
add_test!(single_file, async {
    const FILE_NAME: &str = "single.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Use xzcat
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzcat preserves original file
add_test!(preserves_original, async {
    const FILE_NAME: &str = "preserve.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Use xzcat
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(output.status.success());

    // Compressed file must still exist
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});

// Test xzcat with empty file
add_test!(empty_file, async {
    const FILE_NAME: &str = "empty.txt";

    let mut fixture = Fixture::with_file(FILE_NAME, b"");

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Use xzcat
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
});

// Test xzcat with binary data
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

    // Use xzcat
    let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == binary_data);
});

// Test xzcat is equivalent to xz -dc
add_test!(equivalent_to_xz_dc, async {
    const FILE_NAME: &str = "equiv.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Use xzcat
    let xzcat_output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
    assert!(xzcat_output.status.success());

    // Use xz -dc
    let xz_output = fixture
        .run_cargo("xz", &["-d", "-c", &compressed_path])
        .await;
    assert!(xz_output.status.success());

    // Both outputs should be identical
    assert!(xzcat_output.stdout == xz_output.stdout);
});
