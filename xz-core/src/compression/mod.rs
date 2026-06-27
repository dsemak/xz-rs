mod options;

use lzma_safe::{AloneEncoder, Encoder, RawEncoder};

pub use options::lzma1;
pub use options::{
    BcjOptions, Compression, DeltaOptions, FilterConfig, FilterOptions, FilterType, IntegrityCheck,
    LzmaOptions, Options,
};

/// Encoder built from [`Options`].
pub(crate) enum BuiltEncoder {
    Xz(Encoder),
    Lzma(AloneEncoder),
    Raw(RawEncoder),
}

impl BuiltEncoder {
    pub(crate) fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        action: lzma_safe::Action,
    ) -> std::result::Result<(usize, usize), lzma_safe::Error> {
        match self {
            BuiltEncoder::Xz(enc) => enc.process(input, output, action),
            BuiltEncoder::Lzma(enc) => enc.process(input, output, action),
            BuiltEncoder::Raw(enc) => enc.process(input, output, action),
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        match self {
            BuiltEncoder::Xz(enc) => enc.is_finished(),
            BuiltEncoder::Lzma(enc) => enc.is_finished(),
            BuiltEncoder::Raw(enc) => enc.is_finished(),
        }
    }
}
