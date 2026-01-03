//! Parsing helpers for `--lzma1[=OPTS]`.
//!
//! Upstream `xz` accepts a comma-separated list of key=value pairs.
//! This module implements an upstream-compatible parser for the `.lzma` encoder.
//!
//! Examples (CLI):
//!
//! - `xz --format=lzma --lzma1=preset=6e,dict=8MiB,lc=3,lp=0,pb=2 -k FILE`
//! - `xz --format=lzma --lzma1=mode=fast,mf=hc4,nice=64,depth=128 -k FILE`

use std::result;

use xz_core::options::lzma1::{MatchFinder, Mode};
use xz_core::options::Compression;

use crate::Error;

type ParseResult<T> = result::Result<T, Error>;

const DICT_MIN: u32 = 4096;

// Upstream xz limits to 1.5 GiB: (1<<30) + (1<<29).
const DICT_MAX: u32 = (1u32 << 30) + (1u32 << 29);
const LCLP_MAX: u32 = 4;

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
            invalid_option("Options must be 'name=value' pairs separated with commas".to_string())
        })?;
        let key = key.trim();
        let value = value.trim();
        if value.is_empty() {
            return Err(invalid_option(
                "Options must be 'name=value' pairs separated with commas".into(),
            ));
        }

        match key {
            "preset" => out.preset = Some(parse_preset(value)?),
            "dict" => {
                out.dict_size = Some(parse_u32_size_in_range("dict", value, DICT_MIN, DICT_MAX)?);
            }
            "lc" => out.lc = Some(parse_u32_in_range("lc", value, 0, LCLP_MAX)?),
            "lp" => out.lp = Some(parse_u32_in_range("lp", value, 0, LCLP_MAX)?),
            "pb" => out.pb = Some(parse_u32_in_range("pb", value, 0, 4)?),
            "mode" => out.mode = Some(parse_mode(value)?),
            "nice" => out.nice_len = Some(parse_u32_in_range("nice", value, 2, 273)?),
            "mf" => out.mf = Some(parse_mf(value)?),
            "depth" => out.depth = Some(parse_u32("depth", value)?),
            other => return Err(invalid_option(format!("{other}: Invalid option name"))),
        }
    }

    if let (Some(lc), Some(lp)) = (out.lc, out.lp) {
        if lc + lp > LCLP_MAX {
            return Err(invalid_option(
                "The sum of lc and lp must not exceed 4".into(),
            ));
        }
    }

    Ok(out)
}

/// Creates a standardized CLI error for invalid `--lzma1` option strings.
fn invalid_option(message: String) -> Error {
    Error::InvalidOption { message }
}

/// Parse a decimal `u32` value.
fn parse_u32(_name: &str, value: &str) -> ParseResult<u32> {
    let value = value.trim();
    if value == "max" {
        return Ok(u32::MAX);
    }
    value.parse::<u32>().map_err(|_| {
        invalid_option(format!(
            "{value}: Value is not a non-negative decimal integer"
        ))
    })
}

/// Parse a decimal `u32` value and validate it falls within `min..=max`.
fn parse_u32_in_range(name: &str, value: &str, min: u32, max: u32) -> ParseResult<u32> {
    let value = value.trim();
    if value == "max" {
        return Ok(max);
    }

    let parsed = parse_u32(name, value)?;
    if !(min..=max).contains(&parsed) {
        return Err(invalid_option(format!(
            "Value of the option '{name}' must be in the range [{min}, {max}]"
        )));
    }
    Ok(parsed)
}

/// Parse a `u32` byte size value, supporting the same suffix conventions as upstream xz:
/// `KiB`, `MiB`, `GiB`, or `K/M/G` with optional `i`, `iB`, or `B` (all base-2).
fn parse_u32_size_in_range(name: &str, value: &str, min: u32, max: u32) -> ParseResult<u32> {
    let value = value.trim();
    if value == "max" {
        return Ok(max);
    }

    if value.is_empty() {
        return Err(invalid_option(
            "Options must be 'name=value' pairs separated with commas".into(),
        ));
    }

    // Parse leading decimal digits.
    let mut digits_end = 0usize;
    for (idx, ch) in value.char_indices() {
        if !ch.is_ascii_digit() {
            break;
        }
        digits_end = idx + ch.len_utf8();
    }

    if digits_end == 0 {
        return Err(invalid_option(format!(
            "{value}: Value is not a non-negative decimal integer"
        )));
    }

    let num: u64 = value[..digits_end].parse::<u64>().map_err(|_| {
        invalid_option(format!(
            "{value}: Value is not a non-negative decimal integer"
        ))
    })?;

    let suffix = &value[digits_end..];
    let (multiplier, suffix_for_error): (u64, Option<&str>) = if suffix.is_empty() {
        (1, None)
    } else {
        let mut chars = suffix.chars();
        let first = chars.next().unwrap_or('\0');
        let rest = chars.as_str();

        let multiplier = match first {
            'k' | 'K' => 1u64 << 10,
            'm' | 'M' => 1u64 << 20,
            'g' | 'G' => 1u64 << 30,
            _ => 0,
        };

        if multiplier == 0 {
            (0, Some(suffix))
        } else if rest.is_empty() || rest == "i" || rest == "iB" || rest == "B" {
            (multiplier, None)
        } else {
            (0, Some(suffix))
        }
    };

    if multiplier == 0 {
        let bad = suffix_for_error.unwrap_or(suffix);
        return Err(invalid_option(format!(
            "{bad}: Invalid multiplier suffix. Valid suffixes are 'KiB' (2^10), 'MiB' (2^20), and 'GiB' (2^30)."
        )));
    }

    let bytes_u64 = num.checked_mul(multiplier).ok_or_else(|| {
        invalid_option(format!(
            "Value of the option '{name}' must be in the range [{min}, {max}]"
        ))
    })?;

    let bytes_u32 = u32::try_from(bytes_u64).map_err(|_| {
        invalid_option(format!(
            "Value of the option '{name}' must be in the range [{min}, {max}]"
        ))
    })?;

    if !(min..=max).contains(&bytes_u32) {
        return Err(invalid_option(format!(
            "Value of the option '{name}' must be in the range [{min}, {max}]"
        )));
    }

    Ok(bytes_u32)
}

/// Parse `mode=fast|normal`.
fn parse_mode(value: &str) -> ParseResult<Mode> {
    match value {
        "fast" => Ok(Mode::Fast),
        "normal" => Ok(Mode::Normal),
        _ => Err(invalid_option(format!("{value}: Invalid option value"))),
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
        _ => Err(invalid_option(format!("{value}: Invalid option value"))),
    }
}

/// Parse `preset=N` or `preset=Ne` where `N` is `0..=9`.
fn parse_preset(value: &str) -> ParseResult<Compression> {
    // Accept "N" or "Ne" (e.g. "6e") like upstream.
    let (digits, extreme) = value
        .strip_suffix('e')
        .map_or((value, false), |v| (v, true));

    let level: u8 = digits
        .parse::<u8>()
        .map_err(|_| invalid_option(format!("Unsupported LZMA1/LZMA2 preset: {value}")))?;
    if level > 9 {
        return Err(invalid_option(format!(
            "Unsupported LZMA1/LZMA2 preset: {value}"
        )));
    }
    if extreme {
        Ok(Compression::Extreme(level))
    } else {
        Compression::try_from(u32::from(level))
            .map_err(|_| invalid_option(format!("Unsupported LZMA1/LZMA2 preset: {value}")))
    }
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

    /// Test combined invariants match upstream xz: lc + lp must not exceed 4.
    #[test]
    fn parse_rejects_lc_lp_sum_overflow() {
        let err = parse_lzma1_options("lc=4,lp=1").unwrap_err();
        assert!(
            err.to_string()
                .contains("The sum of lc and lp must not exceed 4"),
            "unexpected error: {err}"
        );
    }

    /// Test dictionary limits match upstream xz.
    #[test]
    fn parse_rejects_too_small_dict() {
        let err = parse_lzma1_options("dict=1").unwrap_err();
        assert!(
            err.to_string()
                .contains("Value of the option 'dict' must be in the range"),
            "unexpected error: {err}"
        );
    }

    /// Test size suffixes supported by upstream xz.
    #[test]
    fn parse_dict_accepts_suffixes() {
        let opts = parse_lzma1_options("dict=1MiB").unwrap();
        assert_eq!(opts.dict_size, Some(1024 * 1024));

        let opts = parse_lzma1_options("dict=1MB").unwrap();
        assert_eq!(opts.dict_size, Some(1024 * 1024));

        let opts = parse_lzma1_options("dict=1Gi").unwrap();
        assert_eq!(opts.dict_size, Some(1024 * 1024 * 1024));
    }
}
