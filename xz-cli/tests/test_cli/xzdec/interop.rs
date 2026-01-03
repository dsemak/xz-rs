use crate::add_test;
use crate::common::{generate_random_data, Fixture, SAMPLE_TEXT};
use crate::MB;

// Test xzdec with system xz compressed files
add_test!(system_compressed_files, async {
    const FILE_NAME: &str = "system_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Decompress with our xzdec
        let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);
    }
});

// Test system xzdec with our compressed files
add_test!(system_xzdec_our_files, async {
    const FILE_NAME: &str = "our_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with our xz
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with system xzdec if available
    if let Some(output) = fixture.run_system("xzdec", &[&compressed_path]).await {
        assert!(output.status.success());
        assert!(output.stdout_raw == data);
    }
});

// Test compatibility with different compression levels
add_test!(different_levels_compatibility, async {
    const FILE_NAME: &str = "levels_test.txt";

    let data = generate_random_data(MB);

    for level in [1, 6, 9] {
        let mut fixture = Fixture::with_file(FILE_NAME, &data);

        let file_path = fixture.path(FILE_NAME);
        let compressed_path = fixture.compressed_path(FILE_NAME);

        // Compress with system xz at specific level if available
        if let Some(output) = fixture
            .run_system("xz", &[&format!("-{level}"), &file_path])
            .await
        {
            assert!(
                output.status.success(),
                "System compression at level {level} failed"
            );

            // Decompress with our xzdec
            let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
            assert!(
                output.status.success(),
                "Our xzdec failed for level {level}"
            );
            assert!(output.stdout_raw == data);
        }
    }
});

// Test xzdec vs system xzdec output equivalence
add_test!(output_equivalence, async {
    const FILE_NAME: &str = "equiv_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Decompress with our xzdec
        let our_output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
        assert!(our_output.status.success());

        // Decompress with system xzdec if available
        if let Some(system_output) = fixture.run_system("xzdec", &[&compressed_path]).await {
            assert!(system_output.status.success());

            // Outputs should be identical
            assert!(our_output.stdout == system_output.stdout);
        }
    }
});

// Test extreme compression compatibility
add_test!(extreme_compression_compatibility, async {
    const FILE_NAME: &str = "extreme_test.txt";

    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz extreme mode if available
    if let Some(output) = fixture.run_system("xz", &["-e", "-9", &file_path]).await {
        assert!(output.status.success());

        // Decompress with our xzdec
        let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);
    }
});

// Test large file compatibility
add_test!(large_file_compatibility, async {
    const FILE_NAME: &str = "large_compat.bin";

    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Decompress with our xzdec
        let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);
    }
});

// Test bidirectional compatibility
add_test!(bidirectional_compatibility, async {
    const FILE_NAME_1: &str = "system_file.txt";
    const FILE_NAME_2: &str = "our_file.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_files(&[FILE_NAME_1, FILE_NAME_2], &[data, data]);

    let file_path_1 = fixture.path(FILE_NAME_1);
    let file_path_2 = fixture.path(FILE_NAME_2);
    let compressed_path_1 = fixture.compressed_path(FILE_NAME_1);
    let compressed_path_2 = fixture.compressed_path(FILE_NAME_2);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path_1]).await {
        assert!(output.status.success());

        // Decompress with our xzdec
        let output = fixture.run_cargo("xzdec", &[&compressed_path_1]).await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);

        // Compress with our xz
        let output = fixture.run_cargo("xz", &[&file_path_2]).await;
        assert!(output.status.success());

        // Decompress with system xzdec if available
        if let Some(output) = fixture.run_system("xzdec", &[&compressed_path_2]).await {
            assert!(output.status.success());
            assert!(output.stdout_raw == data);
        }
    }
});

// Test format compatibility with various file types
add_test!(format_compatibility, async {
    const FILES: [&str; 3] = ["test.txt", "test.tar", "test"];

    let data = SAMPLE_TEXT.as_bytes();

    for file_name in FILES {
        let mut fixture = Fixture::with_file(file_name, data);

        let file_path = fixture.path(file_name);
        let compressed_path = fixture.compressed_path(file_name);

        // Compress with system xz if available
        if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
            assert!(output.status.success(), "System xz failed for {file_name}");

            // Decompress with our xzdec
            let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
            assert!(output.status.success(), "Our xzdec failed for {file_name}");
            assert!(output.stdout_raw == data);
        }
    }
});

// Test threaded compression compatibility
add_test!(threaded_compression_compatibility, async {
    const FILE_NAME: &str = "threaded_compat.txt";

    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz using threads if available
    if let Some(output) = fixture.run_system("xz", &["-T4", &file_path]).await {
        assert!(output.status.success());

        // Decompress with our xzdec
        let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);
    }
});

// Test xzdec compatibility with xz utils family
add_test!(xz_utils_family_compatibility, async {
    const FILE_NAME: &str = "xz_utils.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Our xzdec should work
        let output = fixture.run_cargo("xzdec", &[&compressed_path]).await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);

        // System xzcat should also work with same file
        if let Some(output) = fixture.run_system("xzcat", &[&compressed_path]).await {
            assert!(output.status.success());
            assert!(output.stdout_raw == data);
        }
    }
});
