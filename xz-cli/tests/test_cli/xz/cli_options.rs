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
