//! XZ-compressed file diff utility
//!
//! This utility compares XZ-compressed files without explicitly
//! decompressing them to disk.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

use tempfile::NamedTempFile;

use xz_cli::{decompress_file, has_compression_extension, open_input, CliConfig, OperationMode};

const PROGRAM_NAME: &str = "xzdiff";

mod opts;

fn main() {
    match run() {
        Ok(code) => process::exit(code),
        Err(err) => {
            eprintln!("{PROGRAM_NAME}: {err}");
            process::exit(2);
        }
    }
}

/// Execute the `xzdiff` command.
///
/// # Returns
///
/// Returns the exit code to use for the process:
///
/// - `0`: no differences
/// - `1`: differences found
/// - `2`: error (including decompression failures)
fn run() -> Result<i32, String> {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let parsed = opts::parse_args(&args);

    if parsed.show_help {
        print_usage(&parsed.diff_program);
        return Ok(0);
    }

    if parsed.show_version {
        print_version();
        return Ok(0);
    }

    let (file1, file2) = opts::resolve_operands(&parsed.operands)?;

    // Prepare (possibly decompressed) file paths for diff.
    let config = CliConfig {
        mode: OperationMode::Decompress,
        no_warn: false,
        ..CliConfig::default()
    };

    // We keep tempfiles alive until after diff has completed.
    let mut temps: Vec<NamedTempFile> = Vec::new();

    let path1 = materialize_for_diff(&file1, &config, &mut temps)?;
    let path2 = materialize_for_diff(&file2, &config, &mut temps)?;

    let status = run_diff(&parsed.diff_program, &parsed.diff_args, &path1, &path2)
        .map_err(|e| e.to_string())?;

    Ok(status.code().unwrap_or(2))
}

/// Print usage text to stdout.
fn print_usage(diff_program: &OsStr) {
    let diff_display = diff_program.to_string_lossy();
    println!(
        "Usage: xzdiff [OPTION]... FILE1 [FILE2]\n\
Compare FILE1 to FILE2, using their uncompressed contents if they are\n\
compressed. If FILE2 is omitted, then the files compared are FILE1 and\n\
FILE1 from which the compression format suffix has been stripped.\n\n\
Do comparisons like '{diff_display}' does. OPTIONs are the same as for '{diff_display}'.\n",
    );
}

/// Print version text to stdout.
fn print_version() {
    println!("{PROGRAM_NAME} (xz-rs) {}", env!("CARGO_PKG_VERSION"));
}

/// Prepare a path suitable for `diff`.
///
/// If the input looks like a supported compressed file, it is decompressed into a temporary
/// file and the temporary file path is returned. Otherwise the original path is returned.
fn materialize_for_diff(
    path: &OsStr,
    config: &CliConfig,
    temps: &mut Vec<NamedTempFile>,
) -> Result<PathBuf, String> {
    if path == OsStr::new("-") {
        // Supporting '-' is best-effort: allow it only for one side, and pass it to diff.
        // Decompression from stdin is supported by the decompressor, but diff consumes stdin
        // too; mixing both reliably is tricky, so we don't attempt it here.
        return Ok(PathBuf::from("-"));
    }

    let p = Path::new(path);
    if !has_compression_extension(p) {
        return Ok(p.to_path_buf());
    }

    let mut input = open_input(
        p.to_str()
            .ok_or_else(|| "Non-UTF8 paths are not supported".to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let tmp = NamedTempFile::new().map_err(|e| e.to_string())?;
    {
        let mut out = File::create(tmp.path()).map_err(|e| e.to_string())?;
        decompress_file(&mut input, &mut out, config).map_err(|e| e.to_string())?;
    }

    let out_path = tmp.path().to_path_buf();
    temps.push(tmp);
    Ok(out_path)
}

/// Execute `diff` and return its exit status.
fn run_diff(
    diff_program: &OsStr,
    diff_args: &[OsString],
    file1: &Path,
    file2: &Path,
) -> io::Result<process::ExitStatus> {
    let mut cmd = Command::new(diff_program);
    cmd.args(diff_args);
    cmd.arg("--");
    cmd.arg(file1);
    cmd.arg(file2);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    cmd.status()
}
