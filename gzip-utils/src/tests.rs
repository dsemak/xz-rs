/// Basic compression/decompression roundtrip
#[test]
fn gzip_roundtrip() {
    use std::io::Cursor;
    use crate::{compress_file, decompress_file};
    use crate::config::CliConfig;

    let data = b"Test data for gzip";
    let mut compressed = Vec::new();
    let mut decompressed = Vec::new();

    // Compress
    compress_file(
        Cursor::new(data),
        &mut compressed, 
        &CliConfig::default()
    ).unwrap();

    // Decompress
    decompress_file(
        Cursor::new(&compressed),
        &mut decompressed,
        &CliConfig::default()
    ).unwrap();

    assert_eq!(decompressed, data);
}
