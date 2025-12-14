//! Utilities for reading file names from `--files` / `--files0`.
//!
//! The `xz` supports supplying input file names via a separate stream
//! (either newline-delimited or NUL-delimited). This module provides the shared
//! logic to read and parse such lists.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

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
/// - the list contains a file name that is not valid UTF-8
pub fn read_files(path: Option<&str>, delimiter: Delimiter) -> io::Result<Vec<String>> {
    let mut buf = Vec::new();

    match path {
        None | Some("-") => {
            io::stdin().lock().read_to_end(&mut buf)?;
        }
        Some(path) => {
            File::open(Path::new(path))?.read_to_end(&mut buf)?;
        }
    }

    match delimiter {
        Delimiter::Line => parse_line_delimited(&buf),
        Delimiter::Nul => parse_nul_delimited(&buf),
    }
}

fn parse_line_delimited(buf: &[u8]) -> io::Result<Vec<String>> {
    let s = String::from_utf8(buf.to_vec())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut out = Vec::new();
    for line in s.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        out.push(line.to_string());
    }

    Ok(out)
}

fn parse_nul_delimited(buf: &[u8]) -> io::Result<Vec<String>> {
    let mut out = Vec::new();
    for part in buf.split(|b| *b == 0) {
        if part.is_empty() {
            continue;
        }
        let s =
            std::str::from_utf8(part).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        out.push(s.to_string());
    }

    Ok(out)
}
