use crate::add_test;
use crate::common::{generate_random_data, Fixture, SAMPLE_TEXT};
use crate::MB;

// Test xzcat with system xz compressed files
add_test!(system_compressed_files, async {
    const FILE_NAME: &str = "system_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Decompress with our xzcat
        let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);
    }
});

// Test system xzcat with our compressed files
add_test!(system_xzcat_our_files, async {
    const FILE_NAME: &str = "our_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with our xz
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with system xzcat if available
    if let Some(output) = fixture.run_system("xzcat", &[&compressed_path]).await {
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
            .run_system("xz", &[&format!("-{}", level), &file_path])
            .await
        {
            assert!(
                output.status.success(),
                "System compression at level {} failed",
                level
            );

            // Decompress with our xzcat
            let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
            assert!(
                output.status.success(),
                "Our xzcat failed for level {}",
                level
            );
            assert!(output.stdout_raw == data);
        }
    }
});

// Test xzcat vs system xzcat output equivalence
add_test!(output_equivalence, async {
    const FILE_NAME: &str = "equiv_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Decompress with our xzcat
        let our_output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
        assert!(our_output.status.success());

        // Decompress with system xzcat if available
        if let Some(system_output) = fixture.run_system("xzcat", &[&compressed_path]).await {
            assert!(system_output.status.success());

            // Outputs should be identical
            assert!(our_output.stdout == system_output.stdout);
        }
    }
});

// Test xzcat with multiple files compressed by system xz
add_test!(multiple_system_files, async {
    const FILES: [&str; 3] = ["file1.txt", "file2.txt", "file3.txt"];

    let data1 = b"Content 1";
    let data2 = b"Content 2";
    let data3 = b"Content 3";

    let mut fixture = Fixture::with_files(&FILES, &[data1, data2, data3]);

    let mut compressed_paths = Vec::new();

    // Compress all with system xz if available
    let mut all_compressed = true;
    for file in FILES {
        let file_path = fixture.path(file);
        if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
            assert!(output.status.success());
            compressed_paths.push(fixture.compressed_path(file));
        } else {
            all_compressed = false;
            break;
        }
    }

    if all_compressed {
        // Decompress all with our xzcat
        let compressed_refs: Vec<&str> = compressed_paths.iter().map(|s| s.as_str()).collect();
        let output = fixture.run_cargo("xzcat", &compressed_refs).await;
        assert!(output.status.success());

        let expected = b"Content 1Content 2Content 3";
        assert!(output.stdout_raw == expected);
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

        // Decompress with our xzcat
        let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
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

        // Decompress with our xzcat
        let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);
    }
});

// Test format compatibility
add_test!(format_compatibility, async {
    const FILES: [&str; 3] = ["test.txt", "test.tar", "test"];

    let data = SAMPLE_TEXT.as_bytes();

    for file_name in FILES {
        let mut fixture = Fixture::with_file(file_name, data);

        let file_path = fixture.path(file_name);
        let compressed_path = fixture.compressed_path(file_name);

        // Compress with system xz if available
        if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
            assert!(
                output.status.success(),
                "System xz failed for {}",
                file_name
            );

            // Decompress with our xzcat
            let output = fixture.run_cargo("xzcat", &[&compressed_path]).await;
            assert!(
                output.status.success(),
                "Our xzcat failed for {}",
                file_name
            );
            assert!(output.stdout_raw == data);
        }
    }
});

// Test bidirectional compatibility with verbose output
add_test!(verbose_compatibility, async {
    const FILE_NAME: &str = "verbose_compat.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Decompress with our xzcat in verbose mode
        let output = fixture.run_cargo("xzcat", &["-v", &compressed_path]).await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);
    }
});
