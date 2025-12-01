/// Text snippet shared across integration tests.
pub const SAMPLE_TEXT: &str = "The quick brown fox jumps over the lazy dog";

/// Repetitive payload that should compress extremely well.
pub const REPETITIVE_DATA: &str = "ABCABCABCABCABCABCABCABCABCABC";

/// Small binary sample that exercises non-textual workflows.
pub static BINARY_DATA: &[u8] = &[0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE, 0xFD, 0xFC];

/// Generate pseudo-random test data with a fixed seed for determinism.
pub fn generate_random_data(size: usize) -> Vec<u8> {
    generate_random_data_with_seed(size, 12345)
}

/// Generate pseudo-random test data with a caller-provided seed.
pub fn generate_random_data_with_seed(size: usize, mut seed: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);

    for _ in 0..size {
        // Linear congruential generator: compatible, deterministic, dependency-free.
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        data.push((seed >> 16) as u8);
    }

    data
}
