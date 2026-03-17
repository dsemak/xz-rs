const KIB: usize = 1024;
pub const MIB: usize = 1024 * KIB;

#[derive(Clone, Copy)]
pub enum DatasetKind {
    Textual,
    Mixed,
    Binary,
}

impl DatasetKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Textual => "textual",
            Self::Mixed => "mixed",
            Self::Binary => "binary",
        }
    }

    pub fn input_name(self) -> &'static str {
        match self {
            Self::Textual => "payload.txt",
            Self::Mixed => "payload-mixed.bin",
            Self::Binary => "payload-random.bin",
        }
    }

    pub fn build(self, size: usize) -> Vec<u8> {
        match self {
            Self::Textual => build_textual_dataset(size),
            Self::Mixed => build_mixed_dataset(size),
            Self::Binary => build_random_bytes(size, 0xA076_1D64_78BD_642F),
        }
    }
}

fn build_textual_dataset(size: usize) -> Vec<u8> {
    let pattern = b"The quick brown fox jumps over the lazy dog.\n";
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        let remaining = size - data.len();
        let chunk_len = pattern.len().min(remaining);
        data.extend_from_slice(&pattern[..chunk_len]);
    }
    data
}

fn build_mixed_dataset(size: usize) -> Vec<u8> {
    const BLOCK_LEN: usize = 4 * KIB;

    let mut data = Vec::with_capacity(size);
    let text_block = build_textual_dataset(BLOCK_LEN);
    let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
    let mut block_index = 0usize;

    while data.len() < size {
        let remaining = size - data.len();
        let chunk_len = remaining.min(BLOCK_LEN);

        if block_index % 3 == 0 {
            let random_block = build_random_bytes(chunk_len, seed);
            data.extend_from_slice(&random_block);
            seed ^= 0xD134_2543_DE82_EF95;
        } else {
            data.extend_from_slice(&text_block[..chunk_len]);
        }

        block_index += 1;
    }

    data
}

fn build_random_bytes(len: usize, mut seed: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(len);
    for _ in 0..len {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        data.push((seed >> 24) as u8);
    }
    data
}
