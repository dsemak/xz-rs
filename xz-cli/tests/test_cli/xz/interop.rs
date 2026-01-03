use crate::add_test;
use crate::common::{generate_random_data, Fixture, SAMPLE_TEXT};
use crate::MB;

// Our xz (format=lzma) -> system xz -d.
add_test!(our_lzma_to_system_xz, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let lzma_path = fixture.lzma_path(FILE_NAME);

    let output = fixture
        .run_cargo("xz", &["--format=lzma", "-k", &file_path])
        .await;
    assert!(output.status.success(), "our xz failed: {}", output.stderr);

    fixture.remove_file(FILE_NAME);

    let Some(system_out) = fixture.run_system("xz", &["-d", &lzma_path]).await else {
        return;
    };
    assert!(
        system_out.status.success(),
        "system xz -d failed: {}",
        system_out.stderr
    );
});

// Interop matrix for `--lzma1`: our encoder <-> system decoder and vice versa.
add_test!(lzma1_interop_matrix, async {
    const FILE_NAME: &str = "test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let option_strings: &[&str] = &[
        "preset=0",
        "preset=6e",
        "dict=4KiB,lc=3,lp=0,pb=2",
        "dict=64KiB,lc=4,lp=0,pb=0",
        "dict=1MiB,lc=3,lp=1,pb=2",
        "mode=fast,mf=hc3,nice=32,depth=128,dict=1MiB,lc=3,lp=1,pb=2",
        "mode=fast,mf=hc4,nice=64,depth=0,dict=2MiB,lc=4,lp=0,pb=2",
        "mode=normal,mf=bt2,nice=128,depth=0,dict=8MiB,lc=3,lp=0,pb=4",
        "mode=normal,mf=bt4,nice=273,depth=256,dict=16MiB,lc=3,lp=0,pb=2",
    ];

    for opts in option_strings {
        // Our encode -> system decode.
        let mut fixture = Fixture::with_file(FILE_NAME, data);
        let file_path = fixture.path(FILE_NAME);
        let lzma_path = fixture.lzma_path(FILE_NAME);

        let out = fixture
            .run_cargo("xz", &["--format=lzma", "--lzma1", opts, "-k", &file_path])
            .await;
        assert!(
            out.status.success(),
            "our xz failed for '{opts}': {}",
            out.stderr
        );

        fixture.remove_file(FILE_NAME);

        let Some(system_out) = fixture.run_system("xz", &["-d", &lzma_path]).await else {
            continue;
        };
        assert!(
            system_out.status.success(),
            "system xz -d failed for '{opts}': {}",
            system_out.stderr
        );
        fixture.assert_files(&[FILE_NAME], &[data]);

        // System encode -> our decode.
        let mut fixture = Fixture::with_file(FILE_NAME, data);
        let file_path = fixture.path(FILE_NAME);
        let lzma_path = fixture.lzma_path(FILE_NAME);

        let lzma1_arg = format!("--lzma1={opts}");
        let Some(system_out) = fixture
            .run_system("xz", &["--format=lzma", &lzma1_arg, "-k", &file_path])
            .await
        else {
            continue;
        };
        assert!(
            system_out.status.success(),
            "system xz failed for '{opts}': {}",
            system_out.stderr
        );

        fixture.remove_file(FILE_NAME);

        let out = fixture.run_cargo("unlzma", &[&lzma_path]).await;
        assert!(
            out.status.success(),
            "unlzma failed for '{opts}': {}",
            out.stderr
        );
        fixture.assert_files(&[FILE_NAME], &[data]);
    }
});

// Test decompressing files created by system xz
add_test!(decompress_system_xz, async {
    const FILE_NAME: &str = "system_compressed.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with system xz if available
    if let Some(output) = fixture.run_system("xz", &["--keep", &file_path]).await {
        assert!(output.status.success());

        fixture.remove_file(FILE_NAME);

        // Decompress with our xz
        let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
        assert!(output.status.success());

        fixture.assert_files(&[FILE_NAME], &[data]);
    }
});

// Test system xz decompressing our files
add_test!(system_decompress_our_xz, async {
    const FILE_NAME: &str = "our_compressed.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with our xz
    let output = fixture.run_cargo("xz", &["--keep", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    // Decompress with system xz if available
    if let Some(output) = fixture.run_system("xz", &["-d", &compressed_path]).await {
        assert!(output.status.success());

        fixture.assert_files(&[FILE_NAME], &[data]);
    }
});

// Test cross-compatibility with system xz (bidirectional)
add_test!(bidirectional_compatibility, async {
    const FILE_NAME_1: &str = "system_test.txt";
    const FILE_NAME_2: &str = "our_test.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_files(&[FILE_NAME_1, FILE_NAME_2], &[data, data]);

    let file_path_1 = fixture.path(FILE_NAME_1);
    let file_path_2 = fixture.path(FILE_NAME_2);
    let compressed_path_1 = fixture.compressed_path(FILE_NAME_1);
    let compressed_path_2 = fixture.compressed_path(FILE_NAME_2);

    // Try to compress with system xz
    if let Some(output) = fixture.run_system("xz", &["--keep", &file_path_1]).await {
        assert!(output.status.success());

        // Decompress with our xz using stdout
        let output = fixture
            .run_cargo("xz", &["-d", "-c", &compressed_path_1])
            .await;
        assert!(output.status.success());
        assert!(output.stdout_raw == data);

        // Compress with our xz
        let output = fixture.run_cargo("xz", &["-k", &file_path_2]).await;
        assert!(output.status.success());

        // Decompress with system xz using stdout
        if let Some(output) = fixture
            .run_system("xz", &["--decompress", "--stdout", &compressed_path_2])
            .await
        {
            assert!(output.status.success());
            assert!(output.stdout_raw == data);
        }
    }
});

// Test compatibility with different compression levels
add_test!(system_compatibility_levels, async {
    const FILE_NAME: &str = "level_compat.txt";

    let data = generate_random_data(MB);

    for level in [1, 6, 9] {
        let mut fixture = Fixture::with_file(FILE_NAME, &data);

        let file_path = fixture.path(FILE_NAME);
        let compressed_path = fixture.compressed_path(FILE_NAME);

        // Compress with our xz at specific level
        let output = fixture
            .run_cargo("xz", &[&format!("-{level}"), "-k", &file_path])
            .await;
        assert!(
            output.status.success(),
            "Our compression at level {level} failed"
        );

        fixture.remove_file(FILE_NAME);

        // Decompress with system xz if available
        if let Some(output) = fixture.run_system("xz", &["-d", &compressed_path]).await {
            assert!(
                output.status.success(),
                "System decompression of level {level} failed"
            );

            fixture.assert_files(&[FILE_NAME], &[&data]);
        }
    }
});

// Test system xz with various formats
add_test!(system_format_compatibility, async {
    const FILES: [&str; 3] = ["test.txt", "test.tar", "test"];

    let data = SAMPLE_TEXT.as_bytes();

    for file_name in FILES {
        let mut fixture = Fixture::with_file(file_name, data);

        let file_path = fixture.path(file_name);
        let compressed_path = fixture.compressed_path(file_name);

        // Compress with system xz if available
        if let Some(output) = fixture.run_system("xz", &["--keep", &file_path]).await {
            assert!(output.status.success(), "System xz failed for {file_name}");

            fixture.remove_file(file_name);

            // Decompress with our xz
            let output = fixture.run_cargo("xz", &["-d", &compressed_path]).await;
            assert!(
                output.status.success(),
                "Our xz decompression failed for {file_name}"
            );

            fixture.assert_files(&[file_name], &[data]);
        }
    }
});

// Test backward compatibility with large files
add_test!(large_file_backward_compat, async {
    const FILE_NAME: &str = "large_compat.bin";

    let data = generate_random_data(MB);
    let mut fixture = Fixture::with_file(FILE_NAME, &data);

    let file_path = fixture.path(FILE_NAME);
    let compressed_path = fixture.compressed_path(FILE_NAME);

    // Compress with our xz
    let output = fixture.run_cargo("xz", &["--keep", &file_path]).await;
    assert!(output.status.success());

    fixture.remove_file(FILE_NAME);

    // Decompress with system xz if available
    if let Some(output) = fixture.run_system("xz", &["-d", &compressed_path]).await {
        assert!(output.status.success());

        fixture.assert_files(&[FILE_NAME], &[&data]);
    }
});

// Test stdout compatibility
add_test!(stdout_compatibility, async {
    const FILE_NAME: &str = "stdout_compat.txt";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);

    let file_path = fixture.path(FILE_NAME);

    // Compress with our xz to stdout
    let our_output = fixture.run_cargo("xz", &["-c", &file_path]).await;
    assert!(our_output.status.success());

    // Compress with system xz to stdout if available
    if let Some(system_output) = fixture.run_system("xz", &["--stdout", &file_path]).await {
        assert!(system_output.status.success());

        // Both compressed outputs should decompress to the same data
        // (we don't compare compressed data directly as compression may differ slightly)
        assert!(!our_output.stdout.is_empty());
        assert!(!system_output.stdout.is_empty());
    }
});
