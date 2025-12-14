use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::PathBuf;

use tempfile::TempDir;

use super::SparseFileWriter;

fn temp_file(name: &str) -> io::Result<(TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(name);
    Ok((dir, path))
}

/// Sparse writer preserves correct length for all-zero output.
#[test]
fn sparse_writer_sets_len_for_trailing_hole() {
    let (_dir, path) = temp_file("all-zero.tmp").unwrap();

    let file = File::create(&path).unwrap();

    let mut w = SparseFileWriter::new(file);

    let zeros = vec![0u8; 128 * 1024];
    w.write_all(&zeros).unwrap();
    w.flush().unwrap();

    let meta = std::fs::metadata(&path).unwrap();
    assert_eq!(meta.len(), zeros.len() as u64);

    let mut f = File::open(&path).unwrap();
    let mut head = [1u8; 16];
    f.read_exact(&mut head).unwrap();
    assert!(head.iter().all(|&b| b == 0));

    f.seek(SeekFrom::End(-16)).unwrap();
    let mut tail = [1u8; 16];
    f.read_exact(&mut tail).unwrap();
    assert!(tail.iter().all(|&b| b == 0));
}

/// Sparse writer preserves correct contents across a large zero run.
#[test]
fn sparse_writer_keeps_data_around_hole() {
    let (_dir, path) = temp_file("mixed.tmp").unwrap();

    let file = File::create(&path).unwrap();
    let mut w = SparseFileWriter::new(file);

    w.write_all(b"ABC").unwrap();
    let zeros = vec![0u8; 8192];
    w.write_all(&zeros).unwrap();
    w.write_all(b"XYZ").unwrap();
    w.flush().unwrap();

    let expected_len = 3 + 8192 + 3;
    let meta = std::fs::metadata(&path).unwrap();
    assert_eq!(meta.len(), expected_len as u64);

    let mut f = File::open(&path).unwrap();
    let mut buf = [0u8; 3];
    f.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"ABC");

    f.seek(SeekFrom::Start(3 + 123)).unwrap();
    let mut mid = [1u8; 1];
    f.read_exact(&mut mid).unwrap();
    assert_eq!(mid[0], 0);

    f.seek(SeekFrom::End(-3)).unwrap();
    let mut tail = [0u8; 3];
    f.read_exact(&mut tail).unwrap();
    assert_eq!(&tail, b"XYZ");
}

/// Small zero runs below the threshold are written literally.
#[test]
fn sparse_writer_writes_small_zero_runs_literally() {
    let (_dir, path) = temp_file("small-zero.tmp").unwrap();

    let file = File::create(&path).unwrap();
    let mut w = SparseFileWriter::with_threshold(file, 1 << 20);

    let mut data = Vec::new();
    data.extend_from_slice(b"ABC");
    data.extend(std::iter::repeat_n(0u8, 1024));
    data.extend_from_slice(b"XYZ");

    w.write_all(&data).unwrap();
    w.flush().unwrap();

    let mut read_back = Vec::new();
    File::open(&path)
        .unwrap()
        .read_to_end(&mut read_back)
        .unwrap();
    assert_eq!(read_back, data);
}

/// Zero runs spanning multiple writes still produce correct output.
#[test]
fn sparse_writer_handles_zero_runs_split_across_writes() {
    let (_dir, path) = temp_file("split.tmp").unwrap();

    let file = File::create(&path).unwrap();
    let mut w = SparseFileWriter::with_threshold(file, 64);

    w.write_all(b"ABC").unwrap();
    w.write_all(&[0u8; 32]).unwrap();
    w.write_all(&[0u8; 32]).unwrap();
    w.write_all(b"XYZ").unwrap();
    w.flush().unwrap();

    let expected_len = 3 + 64 + 3;
    let meta = std::fs::metadata(&path).unwrap();
    assert_eq!(meta.len(), expected_len as u64);

    let mut f = File::open(&path).unwrap();
    f.seek(SeekFrom::Start(3)).unwrap();
    let mut zeros = vec![1u8; 64];
    f.read_exact(&mut zeros).unwrap();
    assert!(zeros.iter().all(|&b| b == 0));
}
