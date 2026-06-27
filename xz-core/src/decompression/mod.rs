mod options;

use lzma_safe::{Decoder, RawDecoder};

pub use lzma_safe::decoder::options::Flags;
pub use crate::compression::lzma1;
pub use options::Options;

/// Decoder built from [`Options`].
pub(crate) enum BuiltDecoder {
    Standard(Decoder),
    Raw(RawDecoder),
}

impl BuiltDecoder {
    pub(crate) fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        action: lzma_safe::Action,
    ) -> std::result::Result<(usize, usize), lzma_safe::Error> {
        match self {
            BuiltDecoder::Standard(dec) => dec.process(input, output, action),
            BuiltDecoder::Raw(dec) => dec.process(input, output, action),
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        match self {
            BuiltDecoder::Standard(dec) => dec.is_finished(),
            BuiltDecoder::Raw(dec) => dec.is_finished(),
        }
    }
}
