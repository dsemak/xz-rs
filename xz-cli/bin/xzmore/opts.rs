//! Argument handling for `xzmore`.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// Parsed command-line arguments for `xzmore`.
#[derive(Debug, Clone)]
pub struct ParsedArgs {
    /// Pager binary to execute (defaults to `more`, can be overridden via `PAGER`).
    pub pager_program: OsString,
    /// Options forwarded to the underlying pager invocation.
    pub pager_args: Vec<OsString>,
    /// Input file operands as provided by the user.
    pub files: Vec<PathBuf>,
    /// Whether `--help` (or `--h*`) was requested.
    pub show_help: bool,
    /// Whether `--version` (or `--v*`) was requested.
    pub show_version: bool,
}

/// Parse `xzmore` CLI arguments.
///
/// This intentionally does *not* validate options: unknown flags are forwarded
/// to the pager to match the behavior of wrapper tools like `xzgrep`.
pub fn parse_args(args: &[OsString]) -> ParsedArgs {
    let pager_program = env::var_os("PAGER").unwrap_or_else(|| OsString::from("more"));

    let mut pager_args: Vec<OsString> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut show_help = false;
    let mut show_version = false;

    let mut it = args.iter().cloned().peekable();

    while let Some(arg) = it.peek().cloned() {
        let text = arg.to_string_lossy();
        if text.starts_with("--h") {
            show_help = true;
            it.next();
            continue;
        }
        if text.starts_with("--v") {
            show_version = true;
            it.next();
            continue;
        }

        if arg == OsStr::new("--") {
            it.next();
            break;
        }

        // Stop option parsing at the first non-option, but treat "-" as a file operand.
        if text.starts_with('-') && arg != OsStr::new("-") {
            pager_args.push(arg);
            it.next();
            continue;
        }

        break;
    }

    for arg in it {
        files.push(PathBuf::from(arg));
    }

    ParsedArgs {
        pager_program,
        pager_args,
        files,
        show_help,
        show_version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `-` must be treated as a file operand, not as a pager option.
    #[test]
    fn parse_args_treats_dash_as_operand() {
        let args = vec![OsString::from("-"), OsString::from("file.txt")];
        let parsed = parse_args(&args);

        assert!(parsed.pager_args.is_empty());
        assert!(parsed.files == vec![PathBuf::from("-"), PathBuf::from("file.txt")]);
    }

    /// Options must be forwarded to the pager until the first operand or `--`.
    #[test]
    fn parse_args_splits_options_and_files() {
        let args = vec![
            OsString::from("-F"),
            OsString::from("--"),
            OsString::from("a.xz"),
            OsString::from("b.txt"),
        ];
        let parsed = parse_args(&args);

        assert!(parsed.pager_args == vec![OsString::from("-F")]);
        assert!(parsed.files == vec![PathBuf::from("a.xz"), PathBuf::from("b.txt")]);
    }
}
