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

impl TryFrom<u32> for Compression {
    type Error = std::io::Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Compression::Level0),
            1 => Ok(Compression::Level1),
            2 => Ok(Compression::Level2),
            3 => Ok(Compression::Level3),
            4 => Ok(Compression::Level4),
            5 => Ok(Compression::Level5),
            6 => Ok(Compression::Level6),
            7 => Ok(Compression::Level7),
            8 => Ok(Compression::Level8),
            9 => Ok(Compression::Level9),
            _ => {
                if let Ok(value) = u8::try_from(value) {
                    Ok(Compression::Extreme(value))
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Invalid compression level",
                    ))
                }
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

    /// Tests the conversion of extreme presets to liblzma preset values.
    #[test]
    fn test_to_preset_extreme() {
        let extreme_flag = 1u32 << 31;
        assert_eq!(Compression::Extreme(0).to_preset(), extreme_flag);
        assert_eq!(Compression::Extreme(6).to_preset(), 6 | extreme_flag);
        assert_eq!(Compression::Extreme(9).to_preset(), 9 | extreme_flag);
        // Values above 9 should be clamped to 9
        assert_eq!(Compression::Extreme(15).to_preset(), 9 | extreme_flag);
    }

    /// Tests successful conversion from u32 to Compression for standard levels.
    #[test]
    fn test_try_from_standard_levels() {
        assert_eq!(Compression::try_from(0).unwrap(), Compression::Level0);
        assert_eq!(Compression::try_from(1).unwrap(), Compression::Level1);
        assert_eq!(Compression::try_from(2).unwrap(), Compression::Level2);
        assert_eq!(Compression::try_from(3).unwrap(), Compression::Level3);
        assert_eq!(Compression::try_from(4).unwrap(), Compression::Level4);
        assert_eq!(Compression::try_from(5).unwrap(), Compression::Level5);
        assert_eq!(Compression::try_from(6).unwrap(), Compression::Level6);
        assert_eq!(Compression::try_from(7).unwrap(), Compression::Level7);
        assert_eq!(Compression::try_from(8).unwrap(), Compression::Level8);
        assert_eq!(Compression::try_from(9).unwrap(), Compression::Level9);
    }

    /// Tests conversion from u32 to Compression for extreme levels.
    #[test]
    fn test_try_from_extreme_levels() {
        assert_eq!(Compression::try_from(10).unwrap(), Compression::Extreme(10));
        assert_eq!(Compression::try_from(50).unwrap(), Compression::Extreme(50));
        assert_eq!(
            Compression::try_from(100).unwrap(),
            Compression::Extreme(100)
        );
        assert_eq!(
            Compression::try_from(255).unwrap(),
            Compression::Extreme(255)
        );
    }

    /// Tests error cases for `TryFrom` conversion.
    #[test]
    fn test_try_from_invalid_values() {
        // Values that don't fit in u8 should return an error
        assert!(Compression::try_from(256).is_err());
        assert!(Compression::try_from(1000).is_err());
        assert!(Compression::try_from(u32::MAX).is_err());

        // Check that the error has the correct kind
        let error = Compression::try_from(256).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
