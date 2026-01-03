use crate::add_test;
use crate::common::{generate_random_data, Fixture};
use crate::KB;

// Test xzdec with files compressed at different levels
add_test!(different_compression_levels, async {
    const FILE_NAME: &str = "levels.txt";

    let data = generate_random_data(KB);

    for level in [1, 6, 9] {
        let mut fixture = Fixture::with_file(FILE_NAME, &data);

        let file_path = fixture.path(FILE_NAME);
        let compressed_path = fixture.compressed_path(FILE_NAME);

        // Compress at specific level
        let output = fixture
            .run_cargo("xz", &[&format!("-{level}"), &file_path])
            .await;
        assert!(output.status.success());

        // xzdec should decompress any level
        let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
        assert!(output.status.success(), "Failed for level {level}");
        assert!(output.stdout_raw == data);
    }
});

// Test xzdec with extreme compression
add_test!(extreme_compression, async {
    const FILE_NAME: &str = "extreme.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with extreme mode
    let output = fixture.run_cargo("xz", &["-e", "-9", &file_path]).await;
    assert!(output.status.success());

    // xzdec should decompress
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzdec with threaded compression
add_test!(threaded_compression, async {
    const FILE_NAME: &str = "threaded.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with multiple threads
    let output = fixture.run_cargo("xz", &["-T4", &file_path]).await;
    assert!(output.status.success());

    // xzdec should decompress
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzdec has minimal options (should reject most xz options)
add_test!(minimal_options, async {
    const FILE_NAME: &str = "minimal.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // xzdec should work with just the file argument
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});

// Test xzdec with --help (if supported)
add_test!(help_option, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    // Try --help
    let output = fixture.run_cargo("xzdec", &["--help"]).await;
    assert!(output.status.success());
});

// Test xzdec with --version (if supported)
add_test!(version_option, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    // Try --version
    let output = fixture.run_cargo("xzdec", &["--version"]).await;
    assert!(output.status.success());
});

// Test xzdec with no arguments (reads from stdin, should fail with empty input)
add_test!(no_arguments, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");

    // xzdec without arguments reads from empty stdin - should fail
    let output = fixture.run_cargo("xzdec", &[]).await;
    assert!(!output.status.success());
});

// Test xzdec can decompress files compressed with memory limits
add_test!(memory_limited_compression, async {
    const FILE_NAME: &str = "memlimit.txt";

    let data = generate_random_data(KB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with memory limit
    let output = fixture.run_cargo("xz", &["-M", "1M", &file_path]).await;
    assert!(output.status.success());

    // xzdec should decompress
    let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
    assert!(output.status.success());
    assert!(output.stdout_raw == data);
});
