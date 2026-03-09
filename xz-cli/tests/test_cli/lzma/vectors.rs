use crate::add_test;
use crate::common::{Fixture, Vector};

// Test good `.lzma` vectors.
add_test!(good_decode, async {
    let good_vectors = [
        Vector::bundled("good-known_size-with_eopm.lzma"),
        Vector::bundled("good-known_size-without_eopm.lzma"),
        Vector::bundled("good-unknown_size-with_eopm.lzma"),
    ];

    let mut fixture = Fixture::with_vectors(&good_vectors);

    for vector in good_vectors {
        let vector_path = fixture.path(vector.name());
        let output = fixture.run_cargo("unlzma", &["-c", &vector_path]).await;
        assert!(
            output.status.success(),
            "failed for {}: {}",
            vector.name(),
            output.stderr
        );
        assert!(
            !output.stdout_raw.is_empty(),
            "expected non-empty output for {}",
            vector.name()
        );
    }
});

// Test bad `.lzma` vectors.
add_test!(bad_rejected, async {
    let bad_vectors = [
        Vector::bundled("bad-unknown_size-without_eopm.lzma"),
        Vector::bundled("bad-too_big_size-with_eopm.lzma"),
        Vector::bundled("bad-too_small_size-without_eopm-1.lzma"),
        Vector::bundled("bad-too_small_size-without_eopm-2.lzma"),
        Vector::bundled("bad-too_small_size-without_eopm-3.lzma"),
    ];

    let mut fixture = Fixture::with_vectors(&bad_vectors);

    for vector in bad_vectors {
        let vector_path = fixture.path(vector.name());
        let output = fixture.run_cargo("unlzma", &["-c", &vector_path]).await;
        assert!(
            !output.status.success(),
            "expected failure for {}",
            vector.name()
        );
    }
});
