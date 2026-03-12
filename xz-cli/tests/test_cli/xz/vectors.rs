use crate::add_test;
use crate::common::{Fixture, Vector};

const HELLO_WORLD: &[u8] = b"Hello\nWorld!\n";
const LOREM_PREFIX: &[u8] = b"Lorem ipsum dolor sit amet, cons";
const TIFF_PREFIX: &[u8] = b"II*\0";
const ARM64_PREFIX: &[u8] = b"\x00\x00\x00\x94\xff\xff\xff\x97";

// Test good `.xz` vectors with exact known output.
add_test!(good_xz_vectors_decode_expected_output, async {
    let exact_vectors = vec![
        Vector::bundled("good-0-empty.xz"),
        Vector::bundled("good-0pad-empty.xz"),
        Vector::bundled("good-0cat-empty.xz"),
        Vector::bundled("good-0catpad-empty.xz"),
        Vector::bundled("good-1-check-none.xz"),
        Vector::bundled("good-1-check-crc32.xz"),
        Vector::bundled("good-1-check-crc64.xz"),
        Vector::bundled("good-1-check-sha256.xz"),
        Vector::bundled("good-2-lzma2.xz"),
        Vector::bundled("good-1-block_header-1.xz"),
        Vector::bundled("good-1-block_header-2.xz"),
        Vector::bundled("good-1-block_header-3.xz"),
        Vector::bundled("good-1-empty-bcj-lzma2.xz"),
        Vector::bundled("good-1-lzma2-5.xz"),
    ];
    let expected_output = vec![
        b"".as_slice(),
        b"".as_slice(),
        b"".as_slice(),
        b"".as_slice(),
        HELLO_WORLD,
        HELLO_WORLD,
        HELLO_WORLD,
        HELLO_WORLD,
        HELLO_WORLD,
        HELLO_WORLD,
        HELLO_WORLD,
        HELLO_WORLD,
        b"".as_slice(),
        b"".as_slice(),
    ];

    let mut fixture = Fixture::with_vectors(&exact_vectors);

    for (vector, expected_output) in exact_vectors.iter().zip(expected_output.iter()) {
        let vector_path = fixture.path(vector.name());
        let output = fixture.run_cargo("xz", &["-d", "-c", &vector_path]).await;
        assert!(
            output.status.success(),
            "expected success for {}: {}",
            vector.name(),
            output.stderr
        );
        assert_eq!(
            output.stdout_raw.as_slice(),
            *expected_output,
            "unexpected stdout for {}",
            vector.name(),
        );
    }
});

// Test complex `.xz` vectors that exercise filter chains and larger outputs.
add_test!(good_xz_vectors_decode_complex_payloads, async {
    let lorem_vectors = [
        Vector::bundled("good-1-3delta-lzma2.xz"),
        Vector::bundled("good-1-lzma2-1.xz"),
        Vector::bundled("good-1-lzma2-2.xz"),
        Vector::bundled("good-1-lzma2-3.xz"),
        Vector::bundled("good-1-lzma2-4.xz"),
    ];

    let mut fixture = Fixture::with_vectors(&lorem_vectors);

    let mut lorem_reference: Option<Vec<u8>> = None;
    for vector in &lorem_vectors {
        let vector_path = fixture.path(vector.name());
        let output = fixture.run_cargo("xz", &["-d", "-c", &vector_path]).await;
        assert!(
            output.status.success(),
            "expected success for {}: {}",
            vector.name(),
            output.stderr
        );
        assert_eq!(
            output.stdout_raw.len(),
            457,
            "unexpected stdout length for {}",
            vector.name(),
        );
        assert!(
            output.stdout_raw.starts_with(LOREM_PREFIX),
            "unexpected prefix for {}",
            vector.name(),
        );
        if let Some(reference) = &lorem_reference {
            assert_eq!(
                &output.stdout_raw,
                reference,
                "expected identical output for {}",
                vector.name(),
            );
        } else {
            lorem_reference = Some(output.stdout_raw);
        }
    }

    let arm64_vectors = [
        Vector::bundled("good-1-arm64-lzma2-1.xz"),
        Vector::bundled("good-1-arm64-lzma2-2.xz"),
    ];
    let mut arm64_reference: Option<Vec<u8>> = None;
    for vector in &arm64_vectors {
        let vector_path = vector.source_path();
        let vector_path = vector_path.to_string_lossy().into_owned();
        let output = fixture.run_cargo("xz", &["-d", "-c", &vector_path]).await;
        assert!(
            output.status.success(),
            "expected success for {}: {}",
            vector.name(),
            output.stderr
        );
        assert_eq!(
            output.stdout_raw.len(),
            8576,
            "unexpected stdout length for {}",
            vector.name(),
        );
        assert!(
            output.stdout_raw.starts_with(ARM64_PREFIX),
            "unexpected prefix for {}",
            vector.name(),
        );
        if let Some(reference) = &arm64_reference {
            assert_eq!(
                &output.stdout_raw,
                reference,
                "expected identical output for {}",
                vector.name(),
            );
        } else {
            arm64_reference = Some(output.stdout_raw);
        }
    }

    let tiff_vector = Vector::bundled("good-1-delta-lzma2.tiff.xz");
    let vector_path = tiff_vector.source_path();
    let vector_path = vector_path.to_string_lossy().into_owned();
    let output = fixture.run_cargo("xz", &["-d", "-c", &vector_path]).await;
    assert!(
        output.status.success(),
        "expected success for good-1-delta-lzma2.tiff.xz: {}",
        output.stderr,
    );
    assert_eq!(output.stdout_raw.len(), 929_138);
    assert!(output.stdout_raw.starts_with(TIFF_PREFIX));
});

// Test corrupt `.xz` vectors are rejected during integrity checks.
add_test!(bad_xz_vectors_are_rejected, async {
    let bad_vectors = [
        Vector::bundled("bad-0-backward_size.xz"),
        Vector::bundled("bad-0-empty-truncated.xz"),
        Vector::bundled("bad-0-footer_magic.xz"),
        Vector::bundled("bad-0-header_magic.xz"),
        Vector::bundled("bad-0-nonempty_index.xz"),
        Vector::bundled("bad-0cat-alone.xz"),
        Vector::bundled("bad-0cat-header_magic.xz"),
        Vector::bundled("bad-0catpad-empty.xz"),
        Vector::bundled("bad-0pad-empty.xz"),
        Vector::bundled("bad-1-block_header-1.xz"),
        Vector::bundled("bad-1-block_header-2.xz"),
        Vector::bundled("bad-1-block_header-3.xz"),
        Vector::bundled("bad-1-block_header-4.xz"),
        Vector::bundled("bad-1-block_header-5.xz"),
        Vector::bundled("bad-1-block_header-6.xz"),
        Vector::bundled("bad-1-check-crc32-2.xz"),
        Vector::bundled("bad-1-check-crc32.xz"),
        Vector::bundled("bad-1-check-crc64.xz"),
        Vector::bundled("bad-1-check-sha256.xz"),
        Vector::bundled("bad-1-lzma2-1.xz"),
        Vector::bundled("bad-1-lzma2-10.xz"),
        Vector::bundled("bad-1-lzma2-11.xz"),
        Vector::bundled("bad-1-lzma2-2.xz"),
        Vector::bundled("bad-1-lzma2-3.xz"),
        Vector::bundled("bad-1-lzma2-4.xz"),
        Vector::bundled("bad-1-lzma2-5.xz"),
        Vector::bundled("bad-1-lzma2-6.xz"),
        Vector::bundled("bad-1-lzma2-7.xz"),
        Vector::bundled("bad-1-lzma2-8.xz"),
        Vector::bundled("bad-1-lzma2-9.xz"),
        Vector::bundled("bad-1-stream_flags-1.xz"),
        Vector::bundled("bad-1-stream_flags-2.xz"),
        Vector::bundled("bad-1-stream_flags-3.xz"),
        Vector::bundled("bad-1-vli-1.xz"),
        Vector::bundled("bad-1-vli-2.xz"),
        Vector::bundled("bad-2-compressed_data_padding.xz"),
        Vector::bundled("bad-2-index-1.xz"),
        Vector::bundled("bad-2-index-2.xz"),
        Vector::bundled("bad-2-index-3.xz"),
        Vector::bundled("bad-2-index-4.xz"),
        Vector::bundled("bad-2-index-5.xz"),
        Vector::bundled("bad-3-index-uncomp-overflow.xz"),
    ];

    let mut fixture = Fixture::with_vectors(&bad_vectors);

    for vector in &bad_vectors {
        let vector_path = fixture.path(vector.name());
        let output = fixture.run_cargo("xz", &["-t", &vector_path]).await;
        assert!(
            !output.status.success(),
            "expected rejection for {}",
            vector.name()
        );
        assert!(
            !output.stderr.is_empty(),
            "expected diagnostics for {}",
            vector.name()
        );
    }
});

// Test `xz -l` rejects the index uncompressed-size overflow vector.
add_test!(bad_xz_index_uncomp_overflow_rejected_in_list_mode, async {
    let vector = Vector::bundled("bad-3-index-uncomp-overflow.xz");
    let mut fixture = Fixture::with_vector(&vector);
    let vector_path = fixture.path(vector.name());
    let output = fixture.run_cargo("xz", &["-l", &vector_path]).await;
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
});

// Test unsupported `.xz` vectors that must fail cleanly.
add_test!(unsupported_xz_vectors_fail_cleanly, async {
    let rejected_vectors = [
        Vector::bundled("unsupported-block_header.xz"),
        Vector::bundled("unsupported-filter_flags-1.xz"),
        Vector::bundled("unsupported-filter_flags-2.xz"),
        Vector::bundled("unsupported-filter_flags-3.xz"),
    ];

    let mut fixture = Fixture::with_vectors(&rejected_vectors);

    for vector in &rejected_vectors {
        let vector_path = vector.source_path();
        let vector_path = vector_path.to_string_lossy().into_owned();
        let output = fixture.run_cargo("xz", &["-t", &vector_path]).await;
        assert_eq!(
            output.status.code(),
            Some(1),
            "expected exit code 1 for {}",
            vector.name(),
        );
        assert!(
            output.stderr.contains("Unsupported"),
            "expected unsupported diagnostic for {}: {}",
            vector.name(),
            output.stderr,
        );
    }
});

// Test unsupported integrity check `.xz` vectors warn with exit code 2.
add_test!(unsupported_xz_integrity_check_warns, async {
    let vector = Vector::bundled("unsupported-check.xz");
    let mut fixture = Fixture::with_vector(&vector);
    let vector_path = fixture.path(vector.name());
    let output = fixture.run_cargo("xz", &["-d", "-c", &vector_path]).await;
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout_raw.as_slice(), HELLO_WORLD);
    assert!(
        output
            .stderr
            .contains("Unsupported type of integrity check"),
        "expected unsupported integrity check warning: {}",
        output.stderr,
    );
});

// Test `-qQ` suppresses unsupported-check warning while decoding successfully.
add_test!(
    unsupported_xz_integrity_check_q_q_succeeds_without_warning,
    async {
        let vector = Vector::bundled("unsupported-check.xz");
        let mut fixture = Fixture::with_vector(&vector);
        let vector_path = fixture.path(vector.name());
        let output = fixture
            .run_cargo("xz", &["-d", "-c", "-qQ", &vector_path])
            .await;
        assert!(output.status.success());
        assert_eq!(output.stdout_raw.as_slice(), HELLO_WORLD);
        assert!(output.stderr.is_empty());
    }
);

// Test bad `.lzma` vectors are rejected by `xz -dc`.
add_test!(bad_lzma_vectors_are_rejected_by_xz, async {
    let bad_vectors = [
        Vector::bundled("bad-unknown_size-without_eopm.lzma"),
        Vector::bundled("bad-too_big_size-with_eopm.lzma"),
        Vector::bundled("bad-too_small_size-without_eopm-1.lzma"),
        Vector::bundled("bad-too_small_size-without_eopm-2.lzma"),
        Vector::bundled("bad-too_small_size-without_eopm-3.lzma"),
    ];

    let mut fixture = Fixture::with_vectors(&bad_vectors);

    for vector in &bad_vectors {
        let vector_path = fixture.path(vector.name());
        let output = fixture.run_cargo("xz", &["-d", "-c", &vector_path]).await;
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
    }
});

// Test core `.lz` vectors that `xz` auto-detects and decodes successfully.
add_test!(good_lzip_vectors_decode_expected_output, async {
    let good_vectors = [
        Vector::bundled("good-1-v0.lz"),
        Vector::bundled("good-1-v1.lz"),
        Vector::bundled("good-2-v0-v1.lz"),
        Vector::bundled("good-2-v1-v0.lz"),
        Vector::bundled("good-2-v1-v1.lz"),
    ];

    let mut fixture = Fixture::with_vectors(&good_vectors);

    for vector in &good_vectors {
        let vector_path = fixture.path(vector.name());
        let output = fixture.run_cargo("xz", &["-d", "-c", &vector_path]).await;
        assert!(
            output.status.success(),
            "expected success for {}: {}",
            vector.name(),
            output.stderr
        );
        assert_eq!(output.stdout_raw.as_slice(), HELLO_WORLD);
    }
});

// Test `.lz` v0 with trailing bytes must still decode.
add_test!(lzip_v0_trailing_bytes_are_ignored, async {
    let vector = Vector::bundled("good-1-v0-trailing-1.lz");
    let mut fixture = Fixture::with_vector(&vector);
    let vector_path = fixture.path(vector.name());

    let output = fixture.run_cargo("xz", &["-d", "-c", &vector_path]).await;
    assert!(output.status.success());
    assert_eq!(output.stdout_raw.as_slice(), HELLO_WORLD);
});

// Test corrupt and unsupported `.lz` vectors are rejected.
add_test!(bad_and_unsupported_lzip_vectors_are_rejected, async {
    let rejected_vectors = [
        Vector::bundled("bad-1-v0-uncomp-size.lz"),
        Vector::bundled("bad-1-v1-crc32.lz"),
        Vector::bundled("bad-1-v1-dict-1.lz"),
        Vector::bundled("bad-1-v1-dict-2.lz"),
        Vector::bundled("bad-1-v1-magic-1.lz"),
        Vector::bundled("bad-1-v1-magic-2.lz"),
        Vector::bundled("bad-1-v1-member-size.lz"),
        Vector::bundled("bad-1-v1-trailing-magic.lz"),
        Vector::bundled("bad-1-v1-uncomp-size.lz"),
        Vector::bundled("unsupported-1-v234.lz"),
    ];

    let mut fixture = Fixture::with_vectors(&rejected_vectors);

    for vector in &rejected_vectors {
        let vector_path = fixture.path(vector.name());
        let output = fixture.run_cargo("xz", &["-t", &vector_path]).await;
        assert!(
            !output.status.success(),
            "expected rejection for {}",
            vector.name()
        );
        assert!(
            !output.stderr.is_empty(),
            "expected diagnostics for {}",
            vector.name()
        );
    }
});

// Test `xz -d` auto-decodes upstream `.lz` lzip files.
add_test!(lzip_lz_format_via_xz, async {
    const EXPECTED: &[u8] = b"Hello\nWorld!\n";

    let vector = Vector::bundled("good-1-v1.lz");
    let mut fixture = Fixture::with_vector(&vector);
    let lz_path = fixture.path(vector.name());

    let output = fixture
        .run_cargo("xz", &["-d", "--suffix=.lz", &lz_path])
        .await;
    assert!(output.status.success());
    fixture.assert_files(&["good-1-v1"], &[EXPECTED]);
});
