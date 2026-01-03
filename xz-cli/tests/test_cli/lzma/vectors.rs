use std::path::Path;

use crate::add_test;
use crate::common::Fixture;

fn write_file(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
}

add_test!(lzma_vectors_good_decode, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");
    let root = fixture.root_dir_path().to_path_buf();

    let good_vectors: &[(&str, &[u8])] = &[
        (
            "good-known_size-with_eopm.lzma",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../lzma-safe/liblzma-sys/xz/tests/files/good-known_size-with_eopm.lzma"
            )),
        ),
        (
            "good-known_size-without_eopm.lzma",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../lzma-safe/liblzma-sys/xz/tests/files/good-known_size-without_eopm.lzma"
            )),
        ),
        (
            "good-unknown_size-with_eopm.lzma",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../lzma-safe/liblzma-sys/xz/tests/files/good-unknown_size-with_eopm.lzma"
            )),
        ),
    ];

    for (name, bytes) in good_vectors {
        let path = root.join(name);
        write_file(&path, bytes);

        let output = fixture
            .run_cargo("unlzma", &["-c", path.to_string_lossy().as_ref()])
            .await;
        assert!(
            output.status.success(),
            "failed for {name}: {}",
            output.stderr
        );
        assert!(
            !output.stdout_raw.is_empty(),
            "expected non-empty output for {name}"
        );
    }
});

add_test!(lzma_vectors_bad_rejected, async {
    let mut fixture = Fixture::with_file("dummy.txt", b"dummy");
    let root = fixture.root_dir_path().to_path_buf();

    let bad_vectors: &[(&str, &[u8])] = &[
        (
            "bad-unknown_size-without_eopm.lzma",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../lzma-safe/liblzma-sys/xz/tests/files/bad-unknown_size-without_eopm.lzma"
            )),
        ),
        (
            "bad-too_big_size-with_eopm.lzma",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../lzma-safe/liblzma-sys/xz/tests/files/bad-too_big_size-with_eopm.lzma"
            )),
        ),
        (
            "bad-too_small_size-without_eopm-1.lzma",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../lzma-safe/liblzma-sys/xz/tests/files/bad-too_small_size-without_eopm-1.lzma"
            )),
        ),
        (
            "bad-too_small_size-without_eopm-2.lzma",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../lzma-safe/liblzma-sys/xz/tests/files/bad-too_small_size-without_eopm-2.lzma"
            )),
        ),
        (
            "bad-too_small_size-without_eopm-3.lzma",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../lzma-safe/liblzma-sys/xz/tests/files/bad-too_small_size-without_eopm-3.lzma"
            )),
        ),
    ];

    for (name, bytes) in bad_vectors {
        let path = root.join(name);
        write_file(&path, bytes);

        let output = fixture
            .run_cargo("unlzma", &["-c", path.to_string_lossy().as_ref()])
            .await;
        assert!(!output.status.success(), "expected failure for {name}");
    }
});
