use crate::add_test;
use crate::common::{Fixture, SAMPLE_TEXT};

// Test unlzma skips files with unknown suffix.
add_test!(unknown_suffix_is_skipped, async {
    const FILE_NAME: &str = "no_suffix";

    let data = SAMPLE_TEXT.as_bytes();
    let mut fixture = Fixture::with_file(FILE_NAME, data);
    let file_path = fixture.path(FILE_NAME);

    // unlzma in file->file mode needs to remove a suffix to determine output name.
    // If there is no recognized suffix, it should warn and skip (like upstream xz).
    let out = fixture.run_cargo("unlzma", &[&file_path]).await;
    assert!(!out.status.success());
    assert!(
        out.stderr
            .contains("Filename has an unknown suffix, skipping"),
        "unexpected stderr: {}",
        out.stderr
    );
});
