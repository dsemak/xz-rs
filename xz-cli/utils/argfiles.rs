//! Utilities for reading file names from `--files` / `--files0`.
//!
//! The `xz` supports supplying input file names via a separate stream
//! (either newline-delimited or NUL-delimited). This module provides the shared
//! logic to read and parse such lists.

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

/// Delimiter used to separate file names in an argument file list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    /// One filename per line (`\n`). A trailing `\r` is stripped (CRLF support).
    Line,
    /// NUL-terminated filenames (`\0`).
    Nul,
}

/// Read input file names from a file or from stdin.
///
/// If `path` is `None`, or equals `"-"`, the list is read from stdin.
///
/// # Errors
///
/// Returns an error if:
/// - the list source cannot be read
pub fn read_files(path: Option<&Path>, delimiter: Delimiter) -> io::Result<Vec<PathBuf>> {
    let mut buf = Vec::new();

    if path.is_none() || path == Some(Path::new("-")) {
        io::stdin().lock().read_to_end(&mut buf)?;
    } else if let Some(path) = path {
        File::open(path)?.read_to_end(&mut buf)?;
    }

    match delimiter {
        Delimiter::Line => parse_line_delimited(&buf),
        Delimiter::Nul => parse_nul_delimited(&buf),
    }
}

/// Parse a line-delimited list of file names.
fn parse_line_delimited(buf: &[u8]) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for line in buf.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        out.push(path_from_bytes(line)?);
    }

    Ok(out)
}

/// Parse a NUL-delimited list of file names.
fn parse_nul_delimited(buf: &[u8]) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for part in buf.split(|b| *b == 0) {
        if part.is_empty() {
            continue;
        }
        out.push(path_from_bytes(part)?);
    }

    Ok(out)
}

/// Convert a byte slice to a `PathBuf`.
///
/// # Errors
///
/// Returns an error if the byte slice is not valid UTF-8.
#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> io::Result<PathBuf> {
    Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

/// Convert a byte slice to a `PathBuf`.
///
/// # Errors
///
/// Returns an error if the byte slice is not valid UTF-8.
#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> io::Result<PathBuf> {
    let path = String::from_utf8(bytes.to_vec())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;

    /// Test that line-delimited paths are parsed correctly.
    #[test]
    fn parses_line_delimited_paths_without_extra_copying_through_utf8() {
        let parsed = parse_line_delimited(b"alpha\nbeta\r\ngamma\n").unwrap();
        assert_eq!(
            parsed,
            vec![
                PathBuf::from("alpha"),
                PathBuf::from("beta"),
                PathBuf::from("gamma")
            ]
        );
    }

    /// Test that NUL-delimited paths are parsed correctly.
    #[cfg(unix)]
    #[test]
    fn parses_nul_delimited_non_utf8_paths() {
        let parsed = parse_nul_delimited(b"valid\0bad-\xFF-name\0").unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], PathBuf::from("valid"));
        assert_eq!(parsed[1].as_os_str().as_bytes(), b"bad-\xFF-name");
    }
}
