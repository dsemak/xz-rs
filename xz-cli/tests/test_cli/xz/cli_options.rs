use crate::add_test;
use crate::common::{generate_random_data, Fixture};
use crate::{KB, MB};

// Test all compression levels (1-9)
add_test!(compression_levels, async {
    const FILE_NAME: &str = "levels_test.txt";
    let data = generate_random_data(MB);

    for level in 1..=9 {
        let mut fixture = Fixture::with_file(FILE_NAME, &data);

        let file_path = fixture.path(FILE_NAME);
        let compressed_path = fixture.compressed_path(FILE_NAME);

        // Compress with specific level
        let output = fixture
            .run_cargo("xz", &[&format!("-{}", level), "-k", &file_path])
            .await;
        assert!(output.status.success(), "Level {} failed", level);

        fixture.remove_file(FILE_NAME);

        // Decompress and verify
        let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
        assert!(output.status.success());

        fixture.assert_files(&[FILE_NAME], &[&data]);
    }
});

// Test -T (threads) option
add_test!(thread_option, async {
    const FILE_NAME: &str = "thread_test.txt";
    let data = generate_random_data(MB);

    for threads in [1, 2, 4] {
        let mut fixture = Fixture::with_file(FILE_NAME, &data);

        let file_path = fixture.path(FILE_NAME);
        let compressed_path = fixture.compressed_path(FILE_NAME);

        let output = fixture
            .run_cargo("xz", &[&format!("-T{}", threads), "-k", &file_path])
            .await;
        assert!(output.status.success(), "Thread count {} failed", threads);

        // Verify decompression works
        let output = fixture
            .run_cargo("xz", &["-d", "-f", &compressed_path])
            .await;
        assert!(output.status.success());

        fixture.assert_files(&[FILE_NAME], &[&data]);
    }
});

// Test -M (memory limit) option
add_test!(memory_limit_option, async {
    const FILE_NAME: &str = "memory_test.txt";
    let data = generate_random_data(4 * MB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Test with memory limit
    let output = fixture
        .run_cargo("xz", &["-M", "1M", "-k", &file_path])
        .await;
    assert!(output.status.success());

    // Verify decompression works
    let output = fixture
        .run_cargo("xz", &["-d", "-f", &compressed_path])
        .await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[&data]);
});

// Test -v (verbose) option
add_test!(verbose_option, async {
    const FILE_NAME: &str = "verbose_test.txt";
    let data = generate_random_data(KB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with verbose
    let output = fixture.run_cargo("xz", &["-v", "-k", &file_path]).await;
    assert!(output.status.success());

    // Verbose output should contain information
    assert!(!output.stderr.is_empty() || !output.stdout.is_empty());

    fixture.remove_file(FILE_NAME);

    // Decompress with verbose
    let output = fixture
        .run_cargo("xz", &["-v", "-d", &compressed_path])
        .await;
    assert!(output.status.success());
    assert!(!output.stderr.is_empty() || !output.stdout.is_empty());
});

// Test -q (quiet) option
add_test!(quiet_option, async {
    const FILE_NAME: &str = "quiet_test.txt";
    let data = generate_random_data(KB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);

    // Compress with quiet
    let output = fixture.run_cargo("xz", &["-q", &file_path]).await;
    assert!(output.status.success());

    // No output expected (unless there's an error)
    assert!(output.stderr.is_empty());
});

// Test multiple files compression
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

    // Compress all files at once
    let output = fixture
        .run_cargo("xz", &["-k", &file_path_1, &file_path_2, &file_path_3])
        .await;
    assert!(output.status.success());

    // All compressed files should exist
    assert!(fixture.file_exists(&format!("{}.xz", FILE_1)));
    assert!(fixture.file_exists(&format!("{}.xz", FILE_2)));
    assert!(fixture.file_exists(&format!("{}.xz", FILE_3)));
});

// Test -e (extreme) option
add_test!(extreme_option, async {
    const FILE_NAME: &str = "extreme_test.txt";
    let data = generate_random_data(MB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with extreme mode
    let output = fixture
        .run_cargo("xz", &["-e", "-9", "-k", &file_path])
        .await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    // Verify decompression works
    let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
    assert!(output.status.success());

    fixture.assert_files(&[FILE_NAME], &[&data]);
});

// Test --stdout option (long form)
add_test!(stdout_long_option, async {
    const FILE_NAME: &str = "stdout_long_test.txt";
    let data = generate_random_data(KB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);

    let output = fixture.run_cargo("xz", &["--stdout", &file_path]).await;
    assert!(output.status.success());

    // Original file should still exist
    assert!(fixture.file_exists(FILE_NAME));
    assert!(!output.stdout.is_empty());
});

// Test -S/--suffix option
add_test!(custom_suffix_option, async {
    const FILE_NAME: &str = "suffix_test.txt";
    const CUSTOM_SUFFIX: &str = "custom";
    let data = generate_random_data(KB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);
    let file_path = fixture.path(FILE_NAME);
    let custom_compressed_name = format!("{}.{}", FILE_NAME, CUSTOM_SUFFIX);
    let custom_compressed = fixture.path(&custom_compressed_name);

    // Compress with custom suffix
    let output = fixture
        .run_cargo("xz", &["-S", CUSTOM_SUFFIX, "-k", &file_path])
        .await;
    assert!(output.status.success());

    // Check that the file with custom suffix was created
    assert!(fixture.file_exists(&custom_compressed_name));

    // Original file should still exist (we used -k)
    assert!(fixture.file_exists(FILE_NAME));

    // Remove original before decompression
    fixture.remove_file(FILE_NAME);

    // Decompress with custom suffix
    let output = fixture
        .run_cargo("xz", &["-d", "-S", CUSTOM_SUFFIX, &custom_compressed])
        .await;
    assert!(output.status.success());

    // Verify decompressed content
    fixture.assert_files(&[FILE_NAME], &[&data]);
});

// Test --suffix with dot prefix
add_test!(custom_suffix_with_dot, async {
    const FILE_NAME: &str = "suffix_dot_test.txt";
    const CUSTOM_SUFFIX: &str = ".myext";
    let data = generate_random_data(KB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);
    let file_path = fixture.path(FILE_NAME);
    let custom_compressed_name = format!("{}{}", FILE_NAME, CUSTOM_SUFFIX);
    let custom_compressed = fixture.path(&custom_compressed_name);

    // Compress with custom suffix (with leading dot)
    let output = fixture
        .run_cargo("xz", &["--suffix", CUSTOM_SUFFIX, "-k", &file_path])
        .await;
    assert!(output.status.success());

    // Check that the file with custom suffix was created
    assert!(fixture.file_exists(&custom_compressed_name));

    // Remove original before decompression
    fixture.remove_file(FILE_NAME);

    // Decompress with custom suffix
    let output = fixture
        .run_cargo("xz", &["-d", "--suffix", CUSTOM_SUFFIX, &custom_compressed])
        .await;
    assert!(output.status.success());

    // Verify decompressed content
    fixture.assert_files(&[FILE_NAME], &[&data]);
});

// Test that compressing a file that already has the suffix produces a warning
add_test!(custom_suffix_already_present, async {
    const FILE_NAME: &str = "already.xz";
    const CUSTOM_SUFFIX: &str = "custom";
    let data = generate_random_data(KB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);
    let file_path = fixture.path(FILE_NAME);

    // Try to compress a file that already has .xz extension
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    // Should fail with a warning
    assert!(!output.status.success());

    // Now test with custom suffix
    let custom_file = "test.custom";
    fixture = Fixture::with_file(custom_file, &data);
    let custom_path = fixture.path(custom_file);

    let output = fixture
        .run_cargo("xz", &["-S", CUSTOM_SUFFIX, "-k", &custom_path])
        .await;
    assert!(!output.status.success());
});

// Test --single-stream option decompresses only the first stream
add_test!(single_stream_option, async {
    use std::io::Write;

    const FILE_NAME: &str = "single_stream_test.txt";
    const FILE_NAME_2: &str = "single_stream_test2.txt";
    let data1 = b"First stream data";
    let data2 = b"Second stream data";

    let mut fixture = Fixture::with_file(FILE_NAME, data1);
    let mut fixture2 = Fixture::with_file(FILE_NAME_2, data2);

    let file_path = fixture.path(FILE_NAME);
    let file_path_2 = fixture2.path(FILE_NAME_2);
    let compressed_path = fixture.compressed_path(FILE_NAME);
    let compressed_path_2 = fixture2.compressed_path(FILE_NAME_2);

    // Compress both files
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());
    let output = fixture2.run_cargo("xz", &["-k", &file_path_2]).await;
    assert!(output.status.success());

    // Create a concatenated stream by appending the two compressed files
    let concat_path = fixture.path("concatenated.xz");
    let compressed1 = std::fs::read(&compressed_path).unwrap();
    let compressed2 = std::fs::read(&compressed_path_2).unwrap();
    let mut concat_file = std::fs::File::create(&concat_path).unwrap();
    concat_file.write_all(&compressed1).unwrap();
    concat_file.write_all(&compressed2).unwrap();
    drop(concat_file);

    // Decompress with --single-stream (should only decompress first stream)
    let output = fixture
        .run_cargo("xz", &["-d", "--single-stream", "-c", &concat_path])
        .await;
    assert!(output.status.success());

    // Output should only contain first stream data
    assert_eq!(output.stdout_raw, data1);

    // Decompress without --single-stream (should decompress both streams)
    let output = fixture.run_cargo("xz", &["-d", "-c", &concat_path]).await;
    assert!(output.status.success());

    // Output should contain both stream data concatenated
    let expected_both = [data1.as_slice(), data2.as_slice()].concat();
    assert_eq!(output.stdout_raw, expected_both);
});

// Test --ignore-check option skips integrity verification
add_test!(ignore_check_option, async {
    const FILE_NAME: &str = "ignore_check_test.txt";
    let data = b"Test data for integrity check";

    let mut fixture = Fixture::with_file(FILE_NAME, data);
    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress the file
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    // Decompress with --ignore-check should work
    fixture.remove_file(FILE_NAME);
    let output = fixture
        .run_cargo("xz", &["-d", "--ignore-check", &compressed_path])
        .await;
    assert!(output.status.success());

    // Verify data is correct
    fixture.assert_files(&[FILE_NAME], &[data]);
});
