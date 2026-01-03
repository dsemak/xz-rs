use crate::add_test;
use crate::common::{generate_random_data, Fixture, SAMPLE_TEXT};
use crate::MB;

// Test unxz decompressing system xz files
add_test!(decompress_system_files, async {
    const FILE_NAME: &str = "system_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Decompress with our unxz
        let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
        assert!(output.status.success());

        fixture.assert_files(&[FILE_NAME], &[data]);
    }
});

// Test system unxz decompressing our files
add_test!(system_decompress_our_files, async {
    const FILE_NAME: &str = "our_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with our xz
    let output = fixture.run_cargo("xz", &[&file_path]).await;
    assert!(output.status.success());

    // Decompress with system unxz if available
    if let Some(output) = fixture.run_system("unxz", &[&compressed_path]).await {
        assert!(output.status.success());

        fixture.assert_files(&[FILE_NAME], &[data]);
    }
});

// Test cross-compatibility with stdout
add_test!(stdout_cross_compatibility, async {
    const FILE_NAME: &str = "stdout_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &["-k", &file_path]).await {
        assert!(output.status.success());

        // Decompress to stdout with our unxz
        let output = fixture.run_cargo("unxz", &["-c", &compressed_path]).await;
        assert!(output.status.success());
        fixture.assert_files(&[&file_path], &[data]);
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

            // Decompress with our unxz
            let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
            assert!(
                output.status.success(),
                "Our decompression of level {level} failed"
            );

            fixture.assert_files(&[FILE_NAME], &[&data]);
        }
    }
});

// Test compatibility with extreme compression
add_test!(extreme_compression_compatibility, async {
    const FILE_NAME: &str = "extreme_test.txt";

    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz using extreme mode if available
    if let Some(output) = fixture.run_system("xz", &["-e", "-9", &file_path]).await {
        assert!(output.status.success());

        // Decompress with our unxz
        let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
        assert!(output.status.success());

        fixture.assert_files(&[FILE_NAME], &[&data]);
    }
});

// Test large file compatibility
add_test!(large_file_compatibility, async {
    const FILE_NAME: &str = "large_compat.bin";

    let data = generate_random_data(5 * MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Decompress with our unxz
        let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
        assert!(output.status.success());

        fixture.assert_files(&[FILE_NAME], &[&data]);
    }
});

// Test bidirectional compatibility with keep flag
add_test!(bidirectional_keep_compatibility, async {
    const FILE_NAME_1: &str = "system_keep.txt";
    const FILE_NAME_2: &str = "our_keep.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_files(&[FILE_NAME_1, FILE_NAME_2], &[data, data]);

    let file_path_1 = fixture.path(FILE_NAME_1);
    let file_path_2 = fixture.path(FILE_NAME_2);
    let compressed_path_1 = fixture.compressed_path(FILE_NAME_1);
    let compressed_path_2 = fixture.compressed_path(FILE_NAME_2);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &["-k", &file_path_1]).await {
        assert!(output.status.success());

        // Decompress with our unxz with keep
        let output = fixture
            .run_cargo("unxz", &["-k", "-f", &compressed_path_1])
            .await;
        assert!(output.status.success());
        assert!(fixture.file_exists(FILE_NAME_1));
        assert!(fixture.file_exists(&format!("{FILE_NAME_1}.xz")));

        // Compress with our xz
        let output = fixture.run_cargo("xz", &["-k", "-f", &file_path_2]).await;
        assert!(output.status.success());

        // Decompress with system unxz if available
        if let Some(output) = fixture
            .run_system("unxz", &["-k", "-f", &compressed_path_2])
            .await
        {
            assert!(output.status.success());
            assert!(fixture.file_exists(FILE_NAME_2));
            assert!(fixture.file_exists(&format!("{FILE_NAME_2}.xz")));
        }
    }
});

// Test compatibility with various file formats
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

            // Decompress with our unxz
            let output = fixture.run_cargo("unxz", &[&compressed_path]).await;
            assert!(output.status.success(), "Our unxz failed for {file_name}");

            fixture.assert_files(&[file_name], &[data]);
        }
    }
});

// Test integrity check compatibility
add_test!(test_integrity_compatibility, async {
    const FILE_NAME: &str = "integrity_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &[&file_path]).await {
        assert!(output.status.success());

        // Test integrity with our unxz
        let output = fixture.run_cargo("unxz", &["-t", &compressed_path]).await;
        assert!(output.status.success());

        // File should still be compressed
        assert!(fixture.file_exists(&format!("{FILE_NAME}.xz")));
    }
});
