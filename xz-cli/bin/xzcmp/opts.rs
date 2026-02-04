//! Argument handling for `xzcmp`.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use xz_cli::has_compression_extension;

/// Parsed command-line arguments for `xzcmp`.
#[derive(Debug, Clone)]
pub struct ParsedArgs {
    /// `cmp` binary to execute (defaults to `cmp`, can be overridden via `CMP`).
    pub cmp_program: OsString,
    /// Options forwarded to the underlying `cmp` invocation.
    pub cmp_args: Vec<OsString>,
    /// FILE1 [FILE2] operands as provided by the user.
    pub operands: Vec<OsString>,
    /// Whether `--help` (or `--h*`) was requested.
    pub show_help: bool,
    /// Whether `--version` (or `--v*`) was requested.
    pub show_version: bool,
}

/// Parse `xzcmp` CLI arguments.
///
/// This intentionally does *not* validate options: unknown flags are forwarded to `cmp`
/// to match upstream behavior.
pub fn parse_args(args: &[OsString]) -> ParsedArgs {
    let cmp_program = env::var_os("CMP").unwrap_or_else(|| OsString::from("cmp"));

    let mut cmp_args: Vec<OsString> = Vec::new();
    let mut operands: Vec<OsString> = Vec::new();
    let mut show_help = false;
    let mut show_version = false;

    let mut it = args.iter().cloned().peekable();

    while let Some(arg) = it.peek().cloned() {
        let s = arg.to_string_lossy();
        if s.starts_with("--h") {
            show_help = true;
            it.next();
            continue;
        }
        if s.starts_with("--v") {
            show_version = true;
            it.next();
            continue;
        }

        if arg == OsStr::new("--") {
            it.next();
            break;
        }

        // Stop option parsing at the first non-option, but treat "-" as an operand.
        if s.starts_with('-') && arg != OsStr::new("-") {
            cmp_args.push(arg);
            it.next();
            continue;
        }

        break;
    }

    for arg in it {
        operands.push(arg);
    }

    ParsedArgs {
        cmp_program,
        cmp_args,
        operands,
        show_help,
        show_version,
    }
}

/// Resolve input operands into a `(file1, file2)` pair.
///
/// If only a single operand is provided, the second operand is inferred by stripping
/// a supported compression suffix from `file1` (e.g. `foo.xz -> foo`).
pub fn resolve_operands(operands: &[OsString]) -> Result<(OsString, OsString), String> {
    match operands.len() {
        1 => {
            let file1 = operands[0].clone();
            let file2 = infer_second_operand(&file1)?;
            Ok((file1, file2))
        }
        2 => Ok((operands[0].clone(), operands[1].clone())),
        _ => Err("Invalid number of operands; try 'xzcmp --help' for help".to_string()),
    }
}

/// Infer the second operand when only FILE1 was provided.
///
/// This follows upstream `xzcmp` behavior:
/// - `.xz`/`.lzma` suffix is stripped (case-insensitive).
/// - `.txz`/`.tlz` is mapped to `.tar` (case-insensitive).
fn infer_second_operand(file1: &OsStr) -> Result<OsString, String> {
    let s = file1.to_string_lossy();

    if ends_with_ignore_ascii_case(&s, ".txz") || ends_with_ignore_ascii_case(&s, ".tlz") {
        // Map `.t{x,l}z` -> `.tar` like upstream `xzcmp`.
        //
        // Example: `foo.txz` -> `foo.tar`
        let replaced = s[..s.len().saturating_sub(2)].to_string() + "ar";
        return Ok(OsString::from(replaced));
    }

    let path = Path::new(file1);
    if !has_compression_extension(path) {
        return Err(format!("{s}: Unknown compressed file name suffix"));
    }

    let stem = path
        .file_stem()
        .ok_or_else(|| format!("{s}: Unknown compressed file name suffix"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    Ok(parent.join(stem).into_os_string())
}

/// Returns `true` if `haystack` ends with `needle`, comparing ASCII case-insensitively.
fn ends_with_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack[haystack.len() - needle.len()..].eq_ignore_ascii_case(needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `-` must be treated as an operand, not as a cmp option.
    #[test]
    fn parse_args_treats_dash_as_operand() {
        let args = vec![OsString::from("-"), OsString::from("file.txt")];
        let parsed = parse_args(&args);

        assert!(parsed.cmp_args.is_empty());
        assert!(parsed.operands == vec![OsString::from("-"), OsString::from("file.txt")]);
    }

    /// Options must be forwarded to `cmp` until the first operand or `--`.
    #[test]
    fn parse_args_splits_options_and_operands() {
        let args = vec![
            OsString::from("-s"),
            OsString::from("--"),
            OsString::from("a"),
            OsString::from("b"),
        ];
        let parsed = parse_args(&args);

        assert!(parsed.cmp_args == vec![OsString::from("-s")]);
        assert!(parsed.operands == vec![OsString::from("a"), OsString::from("b")]);
    }

    /// `.xz` suffix should be stripped when inferring the second operand.
    #[test]
    fn infer_second_operand_strips_xz_extension() {
        let out = match infer_second_operand(OsStr::new("foo.txt.xz")) {
            Ok(v) => v,
            Err(err) => panic!("infer_second_operand failed: {err}"),
        };
        assert!(out == OsStr::new("foo.txt"));
    }

    /// `.txz` should map to `.tar` like upstream `xzcmp`.
    #[test]
    fn infer_second_operand_maps_txz_to_tar() {
        let out = match infer_second_operand(OsStr::new("foo.txz")) {
            Ok(v) => v,
            Err(err) => panic!("infer_second_operand failed: {err}"),
        };
        assert!(out == OsStr::new("foo.tar"));
    }
}
