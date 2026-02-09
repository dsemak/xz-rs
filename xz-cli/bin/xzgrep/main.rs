//! XZ-compressed file grep utility
//!
//! This utility searches for patterns in XZ-compressed files without
//! explicitly decompressing them to disk.

use std::env;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

use xz_cli::{has_compression_extension, open_input};
use xz_core::{
    config::DecodeMode,
    options::{DecompressionOptions, Flags},
    pipeline,
};

const PROGRAM_NAME: &str = "xzgrep";

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

/// Execute the `xzgrep` command.
///
/// # Returns
///
/// Returns the exit code to use for the process:
///
/// - `0`: at least one match was found and no errors occurred
/// - `1`: no matches were found and no errors occurred
/// - `2`: error (including decompression failures)
fn run() -> Result<i32, String> {
    let argv0 = env::args_os()
        .next()
        .unwrap_or_else(|| OsString::from(PROGRAM_NAME));
    let args: Vec<OsString> = env::args_os().skip(1).collect();

    let parsed = opts::parse_args(&argv0, &args)?;

    if parsed.show_help {
        print_usage(parsed.grep_program.as_os_str());
        return Ok(0);
    }
    if parsed.show_version {
        print_version(parsed.invoked_as.as_os_str());
        return Ok(0);
    }

    let files = if parsed.files.is_empty() {
        vec![OsString::from("-")]
    } else {
        parsed.files.clone()
    };

    // stdin cannot be meaningfully consumed more than once.
    let stdin_count = files.iter().filter(|f| *f == OsStr::new("-")).count();
    if stdin_count > 0 && files.len() > 1 {
        return Err("'-' can only be used as the sole input".to_string());
    }

    let caps = detect_grep_capabilities(&parsed.grep_program, &parsed.grep_base_args);

    let mut res: i32 = 1; // 1: no matches yet
    for file in &files {
        let file_kind = classify_input(file);

        let need_filename_prefix = if parsed.no_filename {
            false
        } else {
            parsed.with_filename || files.len() > 1
        };

        let r = match file_kind {
            InputKind::Stdin => run_grep_on_stdin(
                &parsed.grep_program,
                &parsed.grep_base_args,
                &parsed.grep_args,
                need_filename_prefix,
            )?,
            InputKind::Plain(path) => run_grep_on_file(
                &parsed.grep_program,
                &parsed.grep_base_args,
                &parsed.grep_args,
                need_filename_prefix,
                &path,
            )?,
            InputKind::Compressed(path) => run_grep_on_compressed_file(
                &parsed.grep_program,
                &parsed.grep_base_args,
                &parsed.grep_args,
                need_filename_prefix,
                &path,
                file,
                &caps,
            )?,
        };

        // If grep failed due to a signal, exit immediately and ignore remaining files.
        if r >= 128 {
            return Ok(r);
        }

        if r >= 2 {
            if res < r {
                res = r;
            }
        } else if r == 0 && res == 1 {
            res = 0;
        }
    }

    Ok(res)
}

/// Print usage text to stdout.
fn print_usage(grep_program: &OsStr) {
    let grep_display = grep_program.to_string_lossy();
    println!(
        "Usage: xzgrep [OPTION]... [-e] PATTERN [FILE]...\n\
Look for instances of PATTERN in the input FILEs, using their\n\
uncompressed contents if they are compressed.\n\n\
OPTIONs are the same as for '{grep_display}'.\n",
    );
}

/// Print version text to stdout.
fn print_version(invoked_as: &OsStr) {
    let invoked = Path::new(invoked_as)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(PROGRAM_NAME);
    println!("{invoked} (xz-rs) {}", env!("CARGO_PKG_VERSION"));
}

/// Capabilities detected for the selected `grep` implementation.
#[derive(Debug, Clone)]
struct GrepCaps {
    /// Whether `grep` supports `--label` to control the displayed filename for stdin input.
    supports_label: bool,
}

/// Detect feature support for the selected `grep` implementation.
///
/// This is used to decide whether we can pass options like `--label` to preserve
/// the original filename for stdin-based input.
fn detect_grep_capabilities(grep_program: &OsStr, grep_base_args: &[OsString]) -> GrepCaps {
    // Probe for `-H --label` support (GNU / *BSD).
    let output = Command::new(grep_program)
        .args(grep_base_args)
        .arg("-H")
        .arg("--label=f")
        .arg("x")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(b"x\n");
            }
            child.wait_with_output()
        });

    let supports_label = match output {
        Ok(out) => out.status.success() && out.stdout == b"f:x\n",
        Err(_) => false,
    };

    GrepCaps { supports_label }
}

/// The resolved kind of an input argument passed to `xzgrep`.
#[derive(Debug, Clone)]
enum InputKind {
    /// Read from standard input (`-`).
    Stdin,
    /// Read directly from an uncompressed file.
    Plain(PathBuf),
    /// Decompress the file and stream its contents to `grep`.
    Compressed(PathBuf),
}

/// Classify an input argument as stdin, plain file, or compressed file.
fn classify_input(file: &OsStr) -> InputKind {
    if file == OsStr::new("-") {
        return InputKind::Stdin;
    }

    let p = Path::new(file);
    if has_compression_extension(p) {
        InputKind::Compressed(p.to_path_buf())
    } else {
        InputKind::Plain(p.to_path_buf())
    }
}

/// Run `grep` directly on stdin (pass-through).
fn run_grep_on_stdin(
    grep_program: &OsStr,
    grep_base_args: &[OsString],
    grep_args: &[OsString],
    need_filename_prefix: bool,
) -> Result<i32, String> {
    let mut cmd = Command::new(grep_program);
    cmd.args(grep_base_args);
    cmd.args(grep_args);
    if need_filename_prefix {
        cmd.arg("-H");
    }
    cmd.arg("--");
    cmd.arg("-");
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    let status = cmd.status().map_err(|e| e.to_string())?;
    Ok(status.code().unwrap_or(2))
}

/// Run `grep` on a plain (uncompressed) file path.
fn run_grep_on_file(
    grep_program: &OsStr,
    grep_base_args: &[OsString],
    grep_args: &[OsString],
    need_filename_prefix: bool,
    path: &Path,
) -> Result<i32, String> {
    let mut cmd = Command::new(grep_program);
    cmd.args(grep_base_args);
    cmd.args(grep_args);
    if need_filename_prefix {
        cmd.arg("-H");
    }
    cmd.arg("--");
    cmd.arg(path);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    let status = cmd.status().map_err(|e| e.to_string())?;
    Ok(status.code().unwrap_or(2))
}

/// Decompress `path` and stream the output into `grep` via stdin.
fn run_grep_on_compressed_file(
    grep_program: &OsStr,
    grep_base_args: &[OsString],
    grep_args: &[OsString],
    need_filename_prefix: bool,
    path: &Path,
    original_label: &OsStr,
    caps: &GrepCaps,
) -> Result<i32, String> {
    let mut input = open_input(
        path.to_str()
            .ok_or_else(|| "Non-UTF8 paths are not supported".to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let mut cmd = Command::new(grep_program);
    cmd.args(grep_base_args);
    cmd.args(grep_args);

    if need_filename_prefix {
        cmd.arg("-H");
    }
    if caps.supports_label && original_label != OsStr::new("-") {
        cmd.arg("--label");
        cmd.arg(original_label);
    }

    // Grep reads decompressed content from stdin.
    cmd.arg("--");
    cmd.arg("-");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;

    let Some(mut child_stdin) = child.stdin.take() else {
        // Invariant: stdin is piped above, so it must be present.
        return Err("internal error: missing grep stdin pipe".to_string());
    };

    let mut options = DecompressionOptions::default();
    options = options.with_mode(DecodeMode::Auto);
    options = options.with_flags(Flags::CONCATENATED);

    // Stream decompression into grep's stdin.
    let decompression = pipeline::decompress(&mut *input, &mut child_stdin, &options);
    match decompression {
        Ok(_) => {}
        Err(xz_core::Error::Io(err)) if err.kind() == io::ErrorKind::BrokenPipe => {
            // Grep may exit early (e.g. `-q`), which closes its stdin.
        }
        Err(err) => {
            // Ensure grep is reaped even if decompression failed.
            drop(child_stdin);
            let _ = child.wait();
            return Err(format!("{}: {err}", path.display()));
        }
    }

    // Close grep stdin and wait for grep to complete.
    drop(child_stdin);
    let status = child.wait().map_err(|e| e.to_string())?;
    Ok(status.code().unwrap_or(2))
}
