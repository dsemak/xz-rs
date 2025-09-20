//! Compression presets exposed by liblzma.

/// Enum mirroring the preset argument passed to liblzma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum Compression {
    /// Level 0 preset (fastest, lowest ratio).
    Level0 = 0,

    /// Level 1 preset.
    Level1 = 1,

    /// Level 2 preset.
    Level2 = 2,

    /// Level 3 preset.
    Level3 = 3,

    /// Level 4 preset.
    Level4 = 4,

    /// Level 5 preset.
    Level5 = 5,

    /// Level 6 preset (liblzma default).
    #[default]
    Level6 = 6,

    /// Level 7 preset.
    Level7 = 7,

    /// Level 8 preset.
    Level8 = 8,

    /// Level 9 preset (slowest, best ratio).
    Level9 = 9,

    /// Extreme variant of a preset. Values above 9 are clamped.
    Extreme(u8),
}

impl Compression {
    /// Bit flag to enable "extreme" compression mode.
    const LZMA_PRESET_EXTREME: u32 = 1u32 << 31;

    /// Convert to the numeric preset expected by liblzma.
    pub fn to_preset(self) -> u32 {
        match self {
            Compression::Level0 => 0,
            Compression::Level1 => 1,
            Compression::Level2 => 2,
            Compression::Level3 => 3,
            Compression::Level4 => 4,
            Compression::Level5 => 5,
            Compression::Level6 => 6,
            Compression::Level7 => 7,
            Compression::Level8 => 8,
            Compression::Level9 => 9,
            Compression::Extreme(level) => {
                // Clamp level to 0..=9 as required by liblzma.
                let level = u32::from(level.min(9));
                level | Self::LZMA_PRESET_EXTREME
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Compression;

    /// Tests the conversion of compression levels to liblzma preset values.
    #[test]
    fn test_to_preset_levels() {
        assert_eq!(Compression::Level0.to_preset(), 0);
        assert_eq!(Compression::Level6.to_preset(), 6);
        assert_eq!(Compression::Level9.to_preset(), 9);
    }

    // Tests the conversion of extreme presets to liblzma preset values.
    #[test]
    fn test_to_preset_extreme() {
        let extreme_flag = 1u32 << 31;
        assert_eq!(Compression::Extreme(0).to_preset(), extreme_flag);
        assert_eq!(Compression::Extreme(6).to_preset(), 6 | extreme_flag);
        assert_eq!(Compression::Extreme(9).to_preset(), 9 | extreme_flag);
        // Values above 9 should be clamped to 9
        assert_eq!(Compression::Extreme(15).to_preset(), 9 | extreme_flag);
    }
}
