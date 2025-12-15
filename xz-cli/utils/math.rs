//! Small numeric helpers.

/// Return `compressed / uncompressed` as a fraction (not a percentage).
pub(crate) fn ratio_fraction(compressed: u64, uncompressed: u64) -> f64 {
    if uncompressed == 0 {
        return 0.0;
    }
    compressed as f64 / uncompressed as f64
}
