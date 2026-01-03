//! Small numeric helpers.

/// Return `compressed / uncompressed` as a fraction (not a percentage).
pub(crate) fn ratio_fraction(compressed: u64, uncompressed: u64) -> f64 {
    if uncompressed == 0 {
        return 0.0;
    }

    let compressed = u128::from(compressed);
    let uncompressed = u128::from(uncompressed);
    let scaled = compressed.saturating_mul(1_000_000);
    let ratio_micro = (scaled + (uncompressed / 2)) / uncompressed;

    let ratio_micro_u32 = match u32::try_from(ratio_micro) {
        Ok(v) => v,
        Err(_) => u32::MAX,
    };
    f64::from(ratio_micro_u32) / 1_000_000.0
}
