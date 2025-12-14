use crate::add_test;
use crate::common::{generate_random_data, BinaryType, Fixture};
use crate::{KB, MB};

// Test all compression levels (0-9)
add_test!(compression_levels, async {
    const FILE_NAME: &str = "levels_test.txt";
    let data = generate_random_data(MB);

    for level in 0..=9 {
        let mut fixture = Fixture::with_file(FILE_NAME, &data);

        let file_path = fixture.path(FILE_NAME);
        let compressed_path = fixture.compressed_path(FILE_NAME);

        // Compress with specific level
        let output = fixture
            .run_cargo("xz", &[&format!("-{}", level), "-k", &file_path])
            .await;
        assert!(output.status.success());

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
        assert!(output.status.success());

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

// Test -qq (double quiet) option suppresses errors too
add_test!(double_quiet_option, async {
    const FILE_NAME: &str = "double_quiet_test.txt";
    let data = generate_random_data(KB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);

    // Compress with double quiet (-qq)
    let output = fixture.run_cargo("xz", &["-qq", &file_path]).await;
    assert!(output.status.success());

    // No output expected at all
    assert!(output.stderr.is_empty());
    assert!(output.stdout.is_empty());
});

// Test -q doesn't suppress runtime errors (but does suppress warnings elsewhere)
add_test!(quiet_does_not_suppress_errors, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    let missing = fixture.path("this-file-does-not-exist.txt");
    let output = fixture.run_cargo("xz", &["-q", &missing]).await;

    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
});

// Test -qq suppresses runtime error messages like upstream xz
add_test!(double_quiet_suppresses_errors, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    let missing = fixture.path("this-file-does-not-exist.txt");
    let output = fixture.run_cargo("xz", &["-qq", &missing]).await;

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
});

// Test -q suppresses warning-like messages ("already has .xz suffix, skipping")
add_test!(quiet_suppresses_already_has_suffix_warning, async {
    const FILE_NAME: &str = "already_warning.xz";
    let data = generate_random_data(KB);

    let mut fixture = Fixture::with_file(FILE_NAME, &data);
    let file_path = fixture.path(FILE_NAME);

    let output = fixture.run_cargo("xz", &["-q", &file_path]).await;
    assert!(!output.status.success());
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

// Test --files[=FILE] reads newline-delimited file names from a file.
add_test!(files_option_reads_list_from_file, async {
    use std::fs;

    const FILE_1: &str = "files_list_input_1.txt";
    const FILE_2: &str = "files_list_input_2.txt";
    const LIST_FILE: &str = "files_list.txt";

    let data1 = generate_random_data(KB);
    let data2 = generate_random_data(KB);

    let mut fixture = Fixture::with_files(&[FILE_1, FILE_2], &[&data1, &data2]);
    let path1 = fixture.path(FILE_1);
    let path2 = fixture.path(FILE_2);

    let list_path = fixture.path(LIST_FILE);
    fs::write(&list_path, format!("{path1}\n{path2}\n")).unwrap();

    let output = fixture
        .run_cargo("xz", &["--files", &list_path, "-k"])
        .await;
    assert!(output.status.success());

    assert!(fixture.file_exists(&format!("{FILE_1}.xz")));
    assert!(fixture.file_exists(&format!("{FILE_2}.xz")));
    assert!(fixture.file_exists(LIST_FILE));
    assert!(!fixture.file_exists(&format!("{LIST_FILE}.xz")));
});

// Test --files reads newline-delimited file names from stdin when FILE is omitted.
add_test!(files_option_reads_list_from_stdin, async {
    const FILE_1: &str = "files_stdin_input_1.txt";
    const FILE_2: &str = "files_stdin_input_2.txt";

    let data1 = generate_random_data(KB);
    let data2 = generate_random_data(KB);

    let mut fixture = Fixture::with_files(&[FILE_1, FILE_2], &[&data1, &data2]);
    let path1 = fixture.path(FILE_1);
    let path2 = fixture.path(FILE_2);

    let stdin = format!("{path1}\n{path2}\n");
    let output = fixture
        .run_with_stdin(
            BinaryType::cargo("xz"),
            &["--files", "-k"],
            Some(vec![stdin.as_str()]),
        )
        .await;
    assert!(output.status.success());

    assert!(fixture.file_exists(&format!("{FILE_1}.xz")));
    assert!(fixture.file_exists(&format!("{FILE_2}.xz")));
});

// Test --files0[=FILE] reads NUL-delimited file names from a file.
add_test!(files0_option_reads_list_from_file, async {
    use std::fs;

    const FILE_1: &str = "files0_list_input_1.txt";
    const FILE_2: &str = "files0_list_input_2.txt";
    const LIST_FILE: &str = "files0_list.bin";

    let data1 = generate_random_data(KB);
    let data2 = generate_random_data(KB);

    let mut fixture = Fixture::with_files(&[FILE_1, FILE_2], &[&data1, &data2]);
    let path1 = fixture.path(FILE_1);
    let path2 = fixture.path(FILE_2);

    let list_path = fixture.path(LIST_FILE);
    let mut list_bytes = Vec::new();
    list_bytes.extend_from_slice(path1.as_bytes());
    list_bytes.push(0);
    list_bytes.extend_from_slice(path2.as_bytes());
    list_bytes.push(0);
    fs::write(&list_path, list_bytes).unwrap();

    let output = fixture
        .run_cargo("xz", &["--files0", &list_path, "-k"])
        .await;
    assert!(output.status.success());

    assert!(fixture.file_exists(&format!("{FILE_1}.xz")));
    assert!(fixture.file_exists(&format!("{FILE_2}.xz")));
    assert!(fixture.file_exists(LIST_FILE));
    assert!(!fixture.file_exists(&format!("{LIST_FILE}.xz")));
});

// Test --files0 reads NUL-delimited file names from stdin when FILE is omitted.
add_test!(files0_option_reads_list_from_stdin, async {
    const FILE_1: &str = "files0_stdin_input_1.txt";
    const FILE_2: &str = "files0_stdin_input_2.txt";

    let data1 = generate_random_data(KB);
    let data2 = generate_random_data(KB);

    let mut fixture = Fixture::with_files(&[FILE_1, FILE_2], &[&data1, &data2]);
    let path1 = fixture.path(FILE_1);
    let path2 = fixture.path(FILE_2);

    let stdin = format!("{path1}\0{path2}\0");
    let output = fixture
        .run_with_stdin(
            BinaryType::cargo("xz"),
            &["--files0", "-k"],
            Some(vec![stdin.as_str()]),
        )
        .await;
    assert!(output.status.success());

    assert!(fixture.file_exists(&format!("{FILE_1}.xz")));
    assert!(fixture.file_exists(&format!("{FILE_2}.xz")));
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

// Test --no-sparse option disables sparse output when decompressing to a file.
add_test!(no_sparse_option_affects_output_allocation, async {
    use std::fs;
    use std::path::Path;

    const FILE_NAME: &str = "no_sparse_test.bin";

    // Create a file with a large zero run that is a good candidate for sparseness.
    let mut data = Vec::new();
    data.extend_from_slice(b"ABC");
    data.extend(std::iter::repeat_n(0u8, 2 * MB));
    data.extend_from_slice(b"XYZ");

    let mut fixture = Fixture::with_file(FILE_NAME, &data);
    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress and keep the original and .xz so we can decompress twice.
    let output = fixture.run_cargo("xz", &["-k", &file_path]).await;
    assert!(output.status.success());

    // Remove original before decompression (like upstream usage patterns).
    fixture.remove_file(FILE_NAME);

    // First decompression: default behavior (sparse enabled).
    let output = fixture
        .run_cargo("xz", &["-d", "-k", "-f", &compressed_path])
        .await;
    assert!(output.status.success());
    fixture.assert_files(&[FILE_NAME], &[&data]);

    let out_path = Path::new(fixture.root_dir_path()).join(FILE_NAME);
    let meta_sparse = fs::metadata(&out_path).unwrap_or_else(|e| {
        panic!("failed to stat decompressed output {out_path:?}: {e}");
    });
    assert_eq!(meta_sparse.len(), data.len() as u64);

    // Remove output and decompress again with --no-sparse.
    fixture.remove_file(FILE_NAME);
    let output = fixture
        .run_cargo("xz", &["-d", "-k", "-f", "--no-sparse", &compressed_path])
        .await;
    assert!(output.status.success());
    fixture.assert_files(&[FILE_NAME], &[&data]);

    let meta_dense = fs::metadata(&out_path).unwrap_or_else(|e| {
        panic!("failed to stat decompressed output {out_path:?}: {e}");
    });
    assert_eq!(meta_dense.len(), data.len() as u64);

    // On filesystems that support sparse files, the default output should allocate
    // fewer (or equal) blocks compared to --no-sparse.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let blocks_sparse = meta_sparse.blocks();
        let blocks_dense = meta_dense.blocks();
        assert!(
            blocks_sparse <= blocks_dense,
            "expected sparse output to allocate <= blocks (sparse={blocks_sparse}, dense={blocks_dense})"
        );
    }
});
