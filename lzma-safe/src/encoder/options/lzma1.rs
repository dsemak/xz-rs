//! LZMA1 encoder options.
//!
//! These settings correspond to `lzma_options_lzma` from liblzma and are used by the legacy
//! `.lzma` (also known as "`LZMA_Alone`") container format.
//!
//! Note that the `.lzma` container supports only LZMA1. There is no integrity check field in the
//! container, and only the actions [`crate::Action::Run`] and [`crate::Action::Finish`] are valid
//! when coding.

use crate::encoder::options::Compression;
use crate::Result;

/// LZMA match finder mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Fast mode (`LZMA_MODE_FAST`).
    Fast,
    /// Normal mode (`LZMA_MODE_NORMAL`).
    Normal,
}

impl From<Mode> for liblzma_sys::lzma_mode {
    fn from(value: Mode) -> Self {
        match value {
            Mode::Fast => liblzma_sys::lzma_mode_LZMA_MODE_FAST,
            Mode::Normal => liblzma_sys::lzma_mode_LZMA_MODE_NORMAL,
        }
    }
}

/// Match finder algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchFinder {
    /// Hash chain match finder (`LZMA_MF_HC3`).
    Hc3,
    /// Hash chain match finder (`LZMA_MF_HC4`).
    Hc4,
    /// Binary tree match finder (`LZMA_MF_BT2`).
    Bt2,
    /// Binary tree match finder (`LZMA_MF_BT3`).
    Bt3,
    /// Binary tree match finder (`LZMA_MF_BT4`).
    Bt4,
}

impl From<MatchFinder> for liblzma_sys::lzma_match_finder {
    fn from(value: MatchFinder) -> Self {
        match value {
            MatchFinder::Hc3 => liblzma_sys::lzma_match_finder_LZMA_MF_HC3,
            MatchFinder::Hc4 => liblzma_sys::lzma_match_finder_LZMA_MF_HC4,
            MatchFinder::Bt2 => liblzma_sys::lzma_match_finder_LZMA_MF_BT2,
            MatchFinder::Bt3 => liblzma_sys::lzma_match_finder_LZMA_MF_BT3,
            MatchFinder::Bt4 => liblzma_sys::lzma_match_finder_LZMA_MF_BT4,
        }
    }
}

/// Encoder options for LZMA1 (`lzma_options_lzma`).
#[derive(Clone)]
pub struct Lzma1Options {
    raw: liblzma_sys::lzma_options_lzma,
}

impl std::fmt::Debug for Lzma1Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lzma1Options")
            .field("dict_size", &self.raw.dict_size)
            .field("lc", &self.raw.lc)
            .field("lp", &self.raw.lp)
            .field("pb", &self.raw.pb)
            .field("mode", &self.raw.mode)
            .field("nice_len", &self.raw.nice_len)
            .field("mf", &self.raw.mf)
            .field("depth", &self.raw.depth)
            .finish()
    }
}

impl Lzma1Options {
    /// Build options from an `xz(1)`-compatible preset (levels 0-9 and extreme flag).
    ///
    /// This uses `lzma_lzma_preset()` as a starting point.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::OptionsError`] if the preset is not supported by the linked liblzma.
    pub fn from_preset(preset: Compression) -> Result<Self> {
        // SAFETY: lzma_options_lzma is a POD type; zeroed init is valid as a baseline.
        let mut raw: liblzma_sys::lzma_options_lzma = unsafe { std::mem::zeroed() };
        crate::ffi::lzma_lzma_preset(&mut raw, preset.to_preset())?;
        Ok(Self { raw })
    }

    /// Dictionary size in bytes.
    #[must_use]
    pub fn with_dict_size(mut self, dict_size: u32) -> Self {
        self.raw.dict_size = dict_size;
        self
    }

    /// Literal context bits (lc).
    #[must_use]
    pub fn with_lc(mut self, lc: u32) -> Self {
        self.raw.lc = lc;
        self
    }

    /// Literal position bits (lp).
    #[must_use]
    pub fn with_lp(mut self, lp: u32) -> Self {
        self.raw.lp = lp;
        self
    }

    /// Position bits (pb).
    #[must_use]
    pub fn with_pb(mut self, pb: u32) -> Self {
        self.raw.pb = pb;
        self
    }

    /// Encoder mode (fast/normal).
    #[must_use]
    pub fn with_mode(mut self, mode: Mode) -> Self {
        self.raw.mode = mode.into();
        self
    }

    /// Nice length.
    #[must_use]
    pub fn with_nice_len(mut self, nice_len: u32) -> Self {
        self.raw.nice_len = nice_len;
        self
    }

    /// Match finder.
    #[must_use]
    pub fn with_match_finder(mut self, mf: MatchFinder) -> Self {
        self.raw.mf = mf.into();
        self
    }

    /// Maximum match finder depth. Use 0 for liblzma defaults.
    #[must_use]
    pub fn with_depth(mut self, depth: u32) -> Self {
        self.raw.depth = depth;
        self
    }

    /// Borrow the raw liblzma options.
    pub(crate) fn as_raw(&self) -> &liblzma_sys::lzma_options_lzma {
        &self.raw
    }
}

impl Default for Lzma1Options {
    fn default() -> Self {
        // Prefer a deterministic default and mirror the default preset used elsewhere.
        Self::from_preset(Compression::Level6).unwrap_or_else(|_| {
            // If the preset isn't supported (unlikely), fall back to a conservative manual config.
            // This fallback is best-effort; liblzma will still validate during encoder init.
            // SAFETY: POD.
            let mut raw: liblzma_sys::lzma_options_lzma = unsafe { std::mem::zeroed() };
            raw.dict_size = 8 * 1024 * 1024;
            raw.lc = 3;
            raw.lp = 0;
            raw.pb = 2;
            raw.mode = liblzma_sys::lzma_mode_LZMA_MODE_NORMAL;
            raw.nice_len = 64;
            raw.mf = liblzma_sys::lzma_match_finder_LZMA_MF_BT4;
            raw.depth = 0;
            Self { raw }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that presets can be turned into LZMA1 options.
    #[test]
    fn preset_produces_lzma1_options() {
        let opts = Lzma1Options::from_preset(Compression::Level3).unwrap();
        assert!(opts.as_raw().dict_size > 0);
        assert!(opts.as_raw().lc <= 8);
        assert!(opts.as_raw().lp <= 4);
        assert!(opts.as_raw().pb <= 4);
    }

    /// Test builder helpers mutate the underlying structure.
    #[test]
    fn builder_helpers_apply_fields() {
        let opts = Lzma1Options::default()
            .with_dict_size(1 << 20)
            .with_lc(4)
            .with_lp(1)
            .with_pb(2)
            .with_mode(Mode::Fast)
            .with_nice_len(32)
            .with_match_finder(MatchFinder::Hc4)
            .with_depth(64);

        let raw = opts.as_raw();
        assert_eq!(raw.dict_size, 1 << 20);
        assert_eq!(raw.lc, 4);
        assert_eq!(raw.lp, 1);
        assert_eq!(raw.pb, 2);
        assert_eq!(raw.mode, liblzma_sys::lzma_mode_LZMA_MODE_FAST);
        assert_eq!(raw.nice_len, 32);
        assert_eq!(raw.mf, liblzma_sys::lzma_match_finder_LZMA_MF_HC4);
        assert_eq!(raw.depth, 64);
    }
}
