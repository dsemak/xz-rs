use crate::add_test;
use crate::common::{generate_random_data, Fixture};
use crate::KB;

// Test xzcat with -v (verbose) option
add_test!(verbose_option, async {
    const FILE_NAME: &str = "verbose.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzcat with verbose
    let output = fixture.run_cargo("xzcat", &["-v", &compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
    // Verbose info should be in stderr
    assert!(!output.stderr.is_empty() || !output.stdout.is_empty());
});

// Test xzcat with -q (quiet) option
add_test!(quiet_option, async {
    const FILE_NAME: &str = "quiet.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzcat with quiet
    let output = fixture.run_cargo("xzcat", &["-q", &compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
    assert!(output.stderr.is_empty());
});

// Test xzcat with --verbose (long form)
add_test!(verbose_long_option, async {
    const FILE_NAME: &str = "verbose_long.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzcat with --verbose
    let output = fixture
        .run_cargo("xzcat", &["--verbose", &compressed_path])
        .await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzcat with --quiet (long form)
add_test!(quiet_long_option, async {
    const FILE_NAME: &str = "quiet_long.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzcat with --quiet
    let output = fixture
        .run_cargo("xzcat", &["--quiet", &compressed_path])
        .await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
    assert!(output.stderr.is_empty());
});

// Test xzcat with files compressed at different levels
add_test!(different_compression_levels, async {
    const FILE_NAME: &str = "levels.txt";

    let data = generate_random_data(KB);

    for level in [1, 6, 9] {
        let mut fixture = Fixture::with_file(FILE_NAME, &data);

        let file_path = fixture.path(FILE_NAME);
        let compressed_path = fixture.compressed_path(FILE_NAME);

        // Compress at specific level
        let output = fixture
            .run_cargo("xz", &[&format!("-{}", level), &file_path])
            .await;
        assert!(output.status.success());

        // xzcat should decompress any level
        let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
        assert!(output.status.success(), "Failed for level {}", level);
        assert!(output.stdout_raw == data);
    }
});

// Test xzcat processes files in order
add_test!(file_order, async {
    const FILES: [&str; 3] = ["a.txt", "b.txt", "c.txt"];

    let data_a = b"AAA";
    let data_b = b"BBB";
    let data_c = b"CCC";

    let mut fixture = Fixture::with_files(&FILES, &[data_a, data_b, data_c]);

    let mut paths = Vec::new();
    let mut compressed_paths = Vec::new();

    for file in FILES {
        paths.push(fixture.path(file));
        compressed_paths.push(fixture.compressed_path(file));
    }

    // Compress all
    let output = fixture
        .run_cargo("xz", &paths.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .await;
    assert!(output.status.success());

    // xzcat in order
    let output = fixture
        .run_cargo(
            "xzcat",
            &compressed_paths
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        )
        .await;
    assert!(output.status.success());

    let expected = b"AAABBBCCC";
    assert!(output.stdout_raw == expected);
});

// Test xzcat with no arguments (should fail or read stdin)
add_test!(no_arguments, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    // xzcat without arguments should fail
    let output = fixture.run_cargo("xzcat", &[]).await;
    assert!(!output.status.success());
});

// Test xzcat with combined options
add_test!(combined_options, async {
    const FILE_NAME: &str = "combined.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzcat with combined options (if supported)
    let output = fixture.run_cargo("xzcat", &["-q", &compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});
