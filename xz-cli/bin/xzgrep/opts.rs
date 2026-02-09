//! Argument handling for `xzgrep`.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Parsed command-line arguments for `xzgrep`.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct ParsedArgs {
    /// How the wrapper was invoked (basename of argv[0]).
    pub invoked_as: OsString,
    /// `grep` binary to execute (defaults to `grep`, can be overridden via `GREP`).
    pub grep_program: OsString,
    /// Extra wrapper-provided grep args (e.g. `-E` for `xzegrep`).
    pub grep_base_args: Vec<OsString>,
    /// Options and pattern specification forwarded to the underlying `grep` invocation.
    pub grep_args: Vec<OsString>,
    /// FILE operands as provided by the user (may be empty, meaning stdin).
    pub files: Vec<OsString>,
    /// Whether `--help` (or `--h*`) was requested.
    pub show_help: bool,
    /// Whether `--version` (or `--v*`) was requested.
    pub show_version: bool,
    /// Whether the user requested `--no-filename` / `-h`.
    pub no_filename: bool,
    /// Whether the user requested `--with-filename` / `-H`.
    pub with_filename: bool,
}

/// Parse `xzgrep` CLI arguments.
///
/// This intentionally does *not* validate options: unknown flags are forwarded to `grep`
/// to match upstream wrapper behavior.
///
/// # Errors
///
/// Returns an error if the pattern is missing.
#[allow(clippy::while_let_on_iterator)]
pub fn parse_args(argv0: &OsStr, args: &[OsString]) -> Result<ParsedArgs, String> {
    let invoked_as = basename(argv0);
    let (grep_base_args, invoked_as) = wrapper_mode_from_argv0(&invoked_as);

    let grep_program = env::var_os("GREP").unwrap_or_else(|| OsString::from("grep"));

    let mut grep_args: Vec<OsString> = Vec::new();
    let mut files: Vec<OsString> = Vec::new();
    let mut show_help = false;
    let mut show_version = false;
    let mut no_filename = false;
    let mut with_filename = false;

    // Whether pattern has already been supplied using `-e`/`-f`/`--regexp`/`--file`.
    let mut have_pat = false;
    // Whether we've already consumed the bare PATTERN operand.
    let mut consumed_bare_pattern = false;

    let mut it = args.iter().cloned().peekable();
    while let Some(arg) = it.next() {
        let s = arg.to_string_lossy();

        if s.starts_with("--h") {
            show_help = true;
            continue;
        }
        if s.starts_with("--v") {
            show_version = true;
            continue;
        }

        if arg == OsStr::new("--") {
            // Everything after `--` is treated as operands.
            while let Some(op) = it.next() {
                if !consumed_bare_pattern && !have_pat {
                    // `-- PATTERN ...`
                    grep_args.push(OsString::from("-e"));
                    grep_args.push(op);
                    consumed_bare_pattern = true;
                } else {
                    files.push(op);
                }
            }
            break;
        }

        // Once the (bare) PATTERN has been consumed, all remaining args are FILE operands.
        if consumed_bare_pattern || have_pat {
            files.push(arg);
            continue;
        }

        // Still in options / pattern parsing stage.
        if is_grep_option(&arg) && arg != OsStr::new("-") {
            track_filename_flags(&arg, &mut no_filename, &mut with_filename);

            // Pattern-providing forms.
            if is_short_opt_with_inline_arg(&arg, b'e') || is_short_opt_with_inline_arg(&arg, b'f')
            {
                have_pat = true;
                grep_args.push(arg);
                continue;
            }
            if arg == OsStr::new("-e") || arg == OsStr::new("-f") {
                have_pat = true;
                let opt = arg;
                let value = it.next().ok_or_else(|| {
                    format!("{} option requires an argument", opt.to_string_lossy())
                })?;
                grep_args.push(opt);
                grep_args.push(value);
                continue;
            }
            if s.starts_with("--regexp=") || s.starts_with("--file=") {
                have_pat = true;
                grep_args.push(arg);
                continue;
            }
            if arg == OsStr::new("--regexp") || arg == OsStr::new("--file") {
                have_pat = true;
                let opt = arg;
                let value = it.next().ok_or_else(|| {
                    format!("{} option requires an argument", opt.to_string_lossy())
                })?;
                grep_args.push(opt);
                grep_args.push(value);
                continue;
            }

            // Generic option (including options that take arguments): forward as-is.
            grep_args.push(arg);
            continue;
        }

        // First non-option is PATTERN (unless already provided via -e/-f/--regexp/--file).
        grep_args.push(OsString::from("-e"));
        grep_args.push(arg);
        consumed_bare_pattern = true;
    }

    if !have_pat && !consumed_bare_pattern && !show_help && !show_version {
        return Err("Missing pattern; try 'xzgrep --help' for help".to_string());
    }

    Ok(ParsedArgs {
        invoked_as,
        grep_program,
        grep_base_args,
        grep_args,
        files,
        show_help,
        show_version,
        no_filename,
        with_filename,
    })
}

/// Return the last path component of `path`.
fn basename(path: &OsStr) -> OsString {
    let p = Path::new(path);
    p.file_name().unwrap_or(path).to_os_string()
}

/// Determine wrapper mode based on the executable name.
fn wrapper_mode_from_argv0(invoked_as: &OsString) -> (Vec<OsString>, OsString) {
    // Match upstream `xzgrep` behavior: if invoked as xzegrep/xzfgrep,
    // prefer the corresponding grep mode.
    let name = invoked_as.to_string_lossy();
    if name.contains("egrep") {
        return (vec![OsString::from("-E")], OsString::from("xzegrep"));
    }
    if name.contains("fgrep") {
        return (vec![OsString::from("-F")], OsString::from("xzfgrep"));
    }
    (Vec::new(), OsString::from("xzgrep"))
}

/// Return `true` if `arg` looks like an option (starts with `-`).
fn is_grep_option(arg: &OsStr) -> bool {
    let s = arg.to_string_lossy();
    s.starts_with('-')
}

/// Track `-h`/`-H` and long equivalents affecting filename prefixing.
fn track_filename_flags(arg: &OsStr, no_filename: &mut bool, with_filename: &mut bool) {
    if arg == OsStr::new("-h") || arg.to_string_lossy().starts_with("--no-f") {
        *no_filename = true;
    }
    if arg == OsStr::new("-H") || arg.to_string_lossy().starts_with("--with-f") {
        *with_filename = true;
    }
}

/// Return `true` for short options with an inline argument (e.g. `-ePAT`).
fn is_short_opt_with_inline_arg(arg: &OsStr, opt: u8) -> bool {
    let bytes = arg.as_encoded_bytes();
    bytes.len() > 2 && bytes[0] == b'-' && bytes[1] == opt
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--` should force treating the next token as pattern.
    #[test]
    fn parse_args_pattern_after_double_dash() {
        let argv0 = OsStr::new("xzgrep");
        let args = vec![
            OsString::from("--"),
            OsString::from("pat"),
            OsString::from("f"),
        ];
        let parsed = parse_args(argv0, &args).unwrap();
        assert_eq!(
            parsed.grep_args,
            [OsString::from("-e"), OsString::from("pat")]
        );
        assert_eq!(parsed.files, [OsString::from("f")]);
    }

    /// Bare PATTERN must be wrapped with `-e`.
    #[test]
    fn parse_args_inserts_e_for_bare_pattern() {
        let parsed = parse_args(
            OsStr::new("xzgrep"),
            &[OsString::from("hello"), OsString::from("file.xz")],
        )
        .unwrap();
        assert_eq!(
            parsed.grep_args,
            [OsString::from("-e"), OsString::from("hello")]
        );
        assert_eq!(parsed.files, [OsString::from("file.xz")]);
    }

    /// `-e` should mark pattern as already provided.
    #[test]
    fn parse_args_recognizes_e_pattern() {
        let parsed = parse_args(
            OsStr::new("xzgrep"),
            &[
                OsString::from("-n"),
                OsString::from("-e"),
                OsString::from("hi"),
                OsString::from("f.xz"),
            ],
        )
        .unwrap();
        assert_eq!(
            parsed.grep_args,
            [
                OsString::from("-n"),
                OsString::from("-e"),
                OsString::from("hi")
            ]
        );
        assert_eq!(parsed.files, [OsString::from("f.xz")]);
    }

    /// Missing pattern should error (unless help/version was requested).
    #[test]
    fn parse_args_missing_pattern_is_error() {
        let err = parse_args(OsStr::new("xzgrep"), &[OsString::from("file.xz")]).unwrap_err();
        assert!(err.contains("Missing pattern"));
    }

    /// `-h`/`-H` should be detected to keep filename prefixing consistent.
    #[test]
    fn parse_args_tracks_filename_flags() {
        let parsed = parse_args(
            OsStr::new("xzgrep"),
            &[
                OsString::from("-H"),
                OsString::from("pat"),
                OsString::from("file"),
            ],
        )
        .unwrap();
        assert!(parsed.with_filename);
        assert!(!parsed.no_filename);

        let parsed = parse_args(
            OsStr::new("xzgrep"),
            &[
                OsString::from("-h"),
                OsString::from("pat"),
                OsString::from("file"),
            ],
        )
        .unwrap();
        assert!(parsed.no_filename);
    }

    /// Invoking as `xzegrep` should inject `-E`.
    #[test]
    fn parse_args_detects_xzegrep_mode() {
        let parsed = parse_args(OsStr::new("/usr/bin/xzegrep"), &[OsString::from("p")]).unwrap();
        assert_eq!(parsed.invoked_as, OsString::from("xzegrep"));
        assert_eq!(parsed.grep_base_args, [OsString::from("-E")]);
    }
}
