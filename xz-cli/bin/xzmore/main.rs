//! XZ-compressed file pager utility.
//!
//! This utility displays compressed files by transparently decompressing them
//! and forwarding the resulting paths to a pager (by default `more`).

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

use tempfile::NamedTempFile;

use xz_cli::{decompress_file, has_compression_extension, open_input, CliConfig, OperationMode};

const PROGRAM_NAME: &str = "xzmore";

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

/// Execute the `xzmore` command.
///
/// # Returns
///
/// Returns the exit code from the selected pager.
fn run() -> Result<i32, String> {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let parsed = opts::parse_args(&args);

    if parsed.show_help {
        print_usage(&parsed.pager_program);
        return Ok(0);
    }

    if parsed.show_version {
        print_version();
        return Ok(0);
    }

    let files = if parsed.files.is_empty() {
        vec![OsString::from("-")]
    } else {
        parsed.files.clone()
    };

    // stdin cannot be meaningfully consumed more than once.
    let stdin_count = files.iter().filter(|file| *file == OsStr::new("-")).count();
    if stdin_count > 0 && files.len() > 1 {
        return Err("'-' can only be used as the sole input".to_string());
    }

    let config = CliConfig {
        mode: OperationMode::Decompress,
        no_warn: false,
        ..CliConfig::default()
    };

    // Keep tempfiles alive while pager is running.
    let mut temps: Vec<NamedTempFile> = Vec::new();
    let mut pager_inputs: Vec<PathBuf> = Vec::new();

    for file in &files {
        let path = prepare_input_for_pager(file, &config, &mut temps)?;
        pager_inputs.push(path);
    }

    let mut cmd = Command::new(&parsed.pager_program);
    cmd.args(&parsed.pager_args);
    cmd.arg("--");
    cmd.args(&pager_inputs);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let status = cmd.status().map_err(|err| err.to_string())?;
    Ok(status.code().unwrap_or(2))
}

/// Print usage text to stdout.
fn print_usage(pager_program: &OsStr) {
    let pager = pager_program.to_string_lossy();
    println!(
        "Usage: xzmore [OPTION]... [FILE]...\n\
Display FILEs in a pager, using their uncompressed contents if they are\n\
compressed. If no FILE is specified, read from standard input.\n\n\
OPTIONs are the same as for '{pager}'.\n",
    );
}

/// Print version text to stdout.
fn print_version() {
    println!("{PROGRAM_NAME} (xz-rs) {}", env!("CARGO_PKG_VERSION"));
}

/// Prepare an input path suitable for the pager.
///
/// If the input looks like a supported compressed file, it is decompressed into
/// a temporary file and that temporary path is returned. Otherwise, the original
/// path is returned.
fn prepare_input_for_pager(
    file: &OsStr,
    config: &CliConfig,
    temps: &mut Vec<NamedTempFile>,
) -> Result<PathBuf, String> {
    if file == OsStr::new("-") {
        return Ok(PathBuf::from("-"));
    }

    let path = Path::new(file);
    if !has_compression_extension(path) {
        return Ok(path.to_path_buf());
    }

    let mut input = open_input(
        path.to_str()
            .ok_or_else(|| "Non-UTF8 paths are not supported".to_string())?,
    )
    .map_err(|err| err.to_string())?;

    let tmp = NamedTempFile::new().map_err(|err| err.to_string())?;
    {
        // `xzmore` always reads from named files here, never stdin.
        let stdin_input = false;

        let mut out = File::create(tmp.path()).map_err(|err| err.to_string())?;
        decompress_file(&mut input, &mut out, config, stdin_input)
            .map_err(|err| err.to_string())?;
    }

    let out_path = tmp.path().to_path_buf();
    temps.push(tmp);
    Ok(out_path)
}
