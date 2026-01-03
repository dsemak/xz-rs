//! Parsing helpers for `--lzma1[=OPTS]`.
//!
//! Upstream `xz` accepts a comma-separated list of key=value pairs.
//! This module implements a compatible subset intended for `.lzma` encoding.

use std::result;

use xz_core::options::lzma1::{MatchFinder, Mode};
use xz_core::options::Compression;

use crate::Error;

type ParseResult<T> = result::Result<T, Error>;

/// Parsed representation of `--lzma1` options.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CliOptions {
    /// Preset compression level (e.g. `6`, `6e`).
    pub preset: Option<Compression>,
    /// Dictionary size in bytes (supports `K/M/G` and `KiB/MiB/GiB` suffixes).
    pub dict_size: Option<u32>,
    /// Number of literal context bits.
    pub lc: Option<u32>,
    /// Number of literal position bits.
    pub lp: Option<u32>,
    /// Number of position bits.
    pub pb: Option<u32>,
    /// Encoder mode (`fast` or `normal`).
    pub mode: Option<Mode>,
    /// Nice length (match length limit).
    pub nice_len: Option<u32>,
    /// Match finder.
    pub mf: Option<MatchFinder>,
    /// Match finder search depth.
    pub depth: Option<u32>,
}

/// Parse an `xz --lzma1` options string.
///
/// The syntax follows upstream `xz`: a comma-separated list of `key=value` pairs.
///
/// # Parameters
///
/// - `input`: The raw `--lzma1` option string (the part after `=`).
///
/// # Returns
///
/// Returns parsed overrides. Missing keys are returned as `None`.
///
/// # Errors
///
/// Returns [`crate::Error::InvalidOption`] when the string is malformed, contains unknown keys,
/// or any value is out of range.
pub fn parse_lzma1_options(input: &str) -> ParseResult<CliOptions> {
    let mut out = CliOptions::default();
    let input = input.trim();
    if input.is_empty() {
        return Ok(out);
    }

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = part.split_once('=').ok_or_else(|| {
            invalid_option(format!(
                "invalid --lzma1 option '{part}': expected key=value"
            ))
        })?;
        let key = key.trim();
        let value = value.trim();
        if value.is_empty() {
            return Err(invalid_option(format!(
                "invalid --lzma1 option '{part}': empty value"
            )));
        }

        match key {
            "preset" => out.preset = Some(parse_preset(value)?),
            "dict" => out.dict_size = Some(parse_u32_size(value)?),
            "lc" => out.lc = Some(parse_u32_in_range("lc", value, 0, 8)?),
            "lp" => out.lp = Some(parse_u32_in_range("lp", value, 0, 4)?),
            "pb" => out.pb = Some(parse_u32_in_range("pb", value, 0, 4)?),
            "mode" => out.mode = Some(parse_mode(value)?),
            "nice" => out.nice_len = Some(parse_u32_in_range("nice", value, 2, 273)?),
            "mf" => out.mf = Some(parse_mf(value)?),
            "depth" => out.depth = Some(parse_u32("depth", value)?),
            other => {
                return Err(invalid_option(format!(
                    "unknown --lzma1 option key '{other}'"
                )))
            }
        }
    }

    Ok(out)
}

/// Creates a standardized CLI error for invalid `--lzma1` option strings.
fn invalid_option(message: String) -> Error {
    Error::InvalidOption { message }
}

/// Parse a decimal `u32` value.
fn parse_u32(name: &str, value: &str) -> ParseResult<u32> {
    value.parse::<u32>().map_err(|_| {
        invalid_option(format!(
            "invalid --lzma1 {name} value '{value}': expected an integer"
        ))
    })
}

/// Parse a decimal `u32` value and validate it falls within `min..=max`.
fn parse_u32_in_range(name: &str, value: &str, min: u32, max: u32) -> ParseResult<u32> {
    let parsed = parse_u32(name, value)?;
    if !(min..=max).contains(&parsed) {
        return Err(invalid_option(format!(
            "invalid --lzma1 {name} value '{value}': expected {min}..={max}"
        )));
    }
    Ok(parsed)
}

/// Parse `mode=fast|normal`.
fn parse_mode(value: &str) -> ParseResult<Mode> {
    match value {
        "fast" => Ok(Mode::Fast),
        "normal" => Ok(Mode::Normal),
        _ => Err(invalid_option(format!(
            "invalid --lzma1 mode '{value}': expected fast|normal"
        ))),
    }
}

/// Parse `mf=hc3|hc4|bt2|bt3|bt4`.
fn parse_mf(value: &str) -> ParseResult<MatchFinder> {
    match value {
        "hc3" => Ok(MatchFinder::Hc3),
        "hc4" => Ok(MatchFinder::Hc4),
        "bt2" => Ok(MatchFinder::Bt2),
        "bt3" => Ok(MatchFinder::Bt3),
        "bt4" => Ok(MatchFinder::Bt4),
        _ => Err(invalid_option(format!(
            "invalid --lzma1 mf '{value}': expected hc3|hc4|bt2|bt3|bt4"
        ))),
    }
}

/// Parse `preset=N` or `preset=Ne` where `N` is `0..=9`.
fn parse_preset(value: &str) -> ParseResult<Compression> {
    // Accept "N" or "Ne" (e.g. "6e") like upstream.
    let (digits, extreme) = value
        .strip_suffix('e')
        .map_or((value, false), |v| (v, true));

    let level: u8 = digits.parse::<u8>().map_err(|_| {
        invalid_option(format!(
            "invalid --lzma1 preset '{value}': expected 0..9 or 0e..9e"
        ))
    })?;
    if level > 9 {
        return Err(invalid_option(format!(
            "invalid --lzma1 preset '{value}': expected 0..9 or 0e..9e"
        )));
    }
    if extreme {
        Ok(Compression::Extreme(level))
    } else {
        Compression::try_from(u32::from(level)).map_err(|_| {
            invalid_option(format!(
                "invalid --lzma1 preset '{value}': expected 0..9 or 0e..9e"
            ))
        })
    }
}

/// Parse a `u32` byte size value, supporting `K/M/G` and `KiB/MiB/GiB` suffixes.
fn parse_u32_size(value: &str) -> ParseResult<u32> {
    let v = value.trim();
    if v.is_empty() {
        return Err(invalid_option("invalid --lzma1 dict size: empty".into()));
    }

    let (number_part, multiplier) = if let Some(rest) = v.strip_suffix("KiB") {
        (rest, 1024_u64)
    } else if let Some(rest) = v.strip_suffix("MiB") {
        (rest, 1024_u64 * 1024)
    } else if let Some(rest) = v.strip_suffix("GiB") {
        (rest, 1024_u64 * 1024 * 1024)
    } else if let Some(rest) = v.strip_suffix('K').or_else(|| v.strip_suffix('k')) {
        (rest, 1024_u64)
    } else if let Some(rest) = v.strip_suffix('M').or_else(|| v.strip_suffix('m')) {
        (rest, 1024_u64 * 1024)
    } else if let Some(rest) = v.strip_suffix('G').or_else(|| v.strip_suffix('g')) {
        (rest, 1024_u64 * 1024 * 1024)
    } else {
        (v, 1_u64)
    };

    let num: u64 = number_part.trim().parse().map_err(|_| {
        invalid_option(format!(
            "invalid --lzma1 dict size '{value}': expected integer optionally suffixed with K/M/G"
        ))
    })?;

    let bytes = num
        .checked_mul(multiplier)
        .ok_or_else(|| invalid_option(format!("invalid --lzma1 dict size '{value}': overflow")))?;

    u32::try_from(bytes)
        .map_err(|_| invalid_option(format!("invalid --lzma1 dict size '{value}': too large")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test parsing of a typical upstream-like `--lzma1` option string.
    #[test]
    fn parse_full_option_string() {
        let opts = parse_lzma1_options(
            "preset=6e,dict=1MiB,lc=4,lp=0,pb=2,mode=fast,nice=64,mf=hc4,depth=128",
        )
        .unwrap_or_else(|e| panic!("parsing must succeed in this test: {e}"));
        assert_eq!(opts.preset, Some(Compression::Extreme(6)));
        assert_eq!(opts.dict_size, Some(1024 * 1024));
        assert_eq!(opts.lc, Some(4));
        assert_eq!(opts.lp, Some(0));
        assert_eq!(opts.pb, Some(2));
        assert_eq!(opts.mode, Some(Mode::Fast));
        assert_eq!(opts.nice_len, Some(64));
        assert_eq!(opts.mf, Some(MatchFinder::Hc4));
        assert_eq!(opts.depth, Some(128));
    }

    /// Test empty string maps to defaults (no overrides).
    #[test]
    fn parse_empty_is_ok() {
        let opts =
            parse_lzma1_options("").unwrap_or_else(|e| panic!("empty input must be ok: {e}"));
        assert_eq!(opts, CliOptions::default());
    }
}
