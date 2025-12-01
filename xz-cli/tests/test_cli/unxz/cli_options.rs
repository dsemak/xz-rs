use crate::add_test;
use crate::common::{generate_random_data, Fixture};
use crate::{KB, MB};

// Test unxz with -v (verbose) option
add_test!(verbose_option, async {
    const FILE_NAME: &str = "verbose_test.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with verbose
    let output = fixture.run_cargo("unxz", &["-v", &compressed_path]).await;
    assert!(output.status.success());

    // Verbose output should contain information
    assert!(!output.stderr.is_empty() || !output.stdout.is_empty());
});

// Test unxz with -q (quiet) option
add_test!(quiet_option, async {
    const FILE_NAME: &str = "quiet_test.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with quiet
    let output = fixture.run_cargo("unxz", &["-q", &compressed_path]).await;
    assert!(output.status.success());

    // No output expected
    assert!(output.stderr.is_empty());
});

// Test unxz with --stdout (long form)
add_test!(stdout_long_option, async {
    const FILE_NAME: &str = "stdout_long.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress to stdout (long form)
    let output = fixture
        .run_cargo("unxz", &["--stdout", &compressed_path])
        .await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test unxz with --keep (long form)
add_test!(keep_long_option, async {
    const FILE_NAME: &str = "keep_long.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with --keep
    let output = fixture
        .run_cargo("unxz", &["--keep", &compressed_path])
        .await;
    assert!(output.status.success());

    // Both files should exist
    assert!(fixture.file_exists(FILE_NAME));
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});

// Test unxz with --force (long form)
add_test!(force_long_option, async {
    const FILE_NAME: &str = "force_long.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress once
    let output = fixture.run_cargo("unxz", &["-k", &compressed_path]).await;
    assert!(output.status.success());

    // Force overwrite
    let output = fixture
        .run_cargo("unxz", &["--force", &compressed_path])
        .await;
    assert!(output.status.success());
});

// Test unxz with files compressed at different levels
add_test!(different_compression_levels, async {
    const FILE_NAME: &str = "level_test.txt";

    let data = generate_random_data(MB);

    for level in [1, 5, 9] {
        let mut fixture = Fixture::with_file(FILE_NAME, &data);

        let file_path = fixture.path(FILE_NAME);
        let compressed_path = fixture.compressed_path(FILE_NAME);

        // Compress with specific level
        let output = fixture
            .run_cargo("xz", &[&format!("-{}", level), &file_path])
            .await;
        assert!(
            output.status.success(),
            "Compression level {} failed",
            level
        );

        // Decompress with unxz (should work for any level)
        let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
        assert!(
            output.status.success(),
            "Decompression of level {} failed",
            level
        );

        fixture.assert_files(&[FILE_NAME], &[&data]);
    }
});

// Test unxz with --test (long form)
add_test!(test_long_option, async {
    const FILE_NAME: &str = "test_long.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Test with long form
    let output = fixture
        .run_cargo("unxz", &["--test", &compressed_path])
        .await;
    assert!(output.status.success());
});

// Test unxz with combined short options
add_test!(combined_short_options, async {
    const FILE_NAME: &str = "combined.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress first
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with combined options (-kv = keep + verbose)
    let output = fixture.run_cargo("unxz", &["-kv", &compressed_path]).await;
    assert!(output.status.success());

    // Both files should exist
    assert!(fixture.file_exists(FILE_NAME));
    assert!(fixture.file_exists(&format!("{}.xz", FILE_NAME)));
});
