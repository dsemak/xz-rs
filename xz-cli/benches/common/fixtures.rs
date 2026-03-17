use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

use super::datasets::DatasetKind;
use super::targets::CodecFormat;

pub struct CompressFixture {
    _temp_dir: TempDir,
    pub input_path: PathBuf,
    pub compressed_path: PathBuf,
}

pub struct DecodeFileFixture {
    _temp_dir: TempDir,
    pub compressed_path: PathBuf,
    pub output_path: PathBuf,
    pub expected_data: Vec<u8>,
}

pub struct DecodeStdoutFixture {
    _temp_dir: TempDir,
    pub compressed_path: PathBuf,
    pub expected_data: Vec<u8>,
}

pub fn run_checked(command: &mut Command, description: &str) -> Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to run {description}: {error}"));

    assert!(
        output.status.success(),
        "{description} failed with status {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    output
}

pub fn prepare_compress_fixture(
    format: CodecFormat,
    dataset_kind: DatasetKind,
    data: &[u8],
) -> CompressFixture {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let input_path = temp_dir.path().join(dataset_kind.input_name());
    let compressed_path = compressed_output_path(&input_path, format);

    fs::write(&input_path, data).expect("failed to write benchmark input");

    CompressFixture {
        _temp_dir: temp_dir,
        input_path,
        compressed_path,
    }
}

pub fn prepare_decode_file_fixture(
    format: CodecFormat,
    dataset_kind: DatasetKind,
    data: &[u8],
    compressor_path: &Path,
    compression_level: u32,
) -> DecodeFileFixture {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let input_path = temp_dir.path().join(dataset_kind.input_name());
    let compressed_path = compressed_output_path(&input_path, format);

    fs::write(&input_path, data).expect("failed to write benchmark input");
    create_compressed_fixture(&input_path, compressor_path, compression_level);

    assert!(
        compressed_path.is_file(),
        "expected compressed fixture at {:?}",
        compressed_path
    );

    fs::remove_file(&input_path).expect("failed to remove input before decode benchmark");

    DecodeFileFixture {
        _temp_dir: temp_dir,
        compressed_path,
        output_path: input_path,
        expected_data: data.to_vec(),
    }
}

pub fn prepare_decode_stdout_fixture(
    format: CodecFormat,
    dataset_kind: DatasetKind,
    data: &[u8],
    compressor_path: &Path,
    compression_level: u32,
) -> DecodeStdoutFixture {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let input_path = temp_dir.path().join(dataset_kind.input_name());
    let compressed_path = compressed_output_path(&input_path, format);

    fs::write(&input_path, data).expect("failed to write benchmark input");
    create_compressed_fixture(&input_path, compressor_path, compression_level);

    assert!(
        compressed_path.is_file(),
        "expected compressed fixture at {:?}",
        compressed_path
    );

    fs::remove_file(&input_path).expect("failed to remove input before decode benchmark");

    DecodeStdoutFixture {
        _temp_dir: temp_dir,
        compressed_path,
        expected_data: data.to_vec(),
    }
}

fn create_compressed_fixture(input_path: &Path, compressor_path: &Path, compression_level: u32) {
    let mut command = Command::new(compressor_path);
    command
        .arg(format!("-{compression_level}"))
        .arg("-k")
        .arg("-f")
        .arg("-T1")
        .arg(input_path);
    run_checked(&mut command, "fixture compressor");
}

fn compressed_output_path(input_path: &Path, format: CodecFormat) -> PathBuf {
    let file_name = input_path
        .file_name()
        .expect("input file should have a name")
        .to_string_lossy();
    input_path.with_file_name(format!("{file_name}.{}", format.extension()))
}
