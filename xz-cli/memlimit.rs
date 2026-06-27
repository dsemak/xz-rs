//! Builder for compression options with upstream-compatible memory-limit handling.

use std::num::NonZeroU64;
use std::time::Duration;

use lzma_safe::encoder::{
    easy_encoder_memusage, filters_encoder_memusage, mt_encoder_memusage, raw_encoder_memusage,
};
use lzma_safe::encoder::options::Options as EncoderMtOptions;
use xz_core::compression::Options as CompressionOptions;
use xz_core::config::EncodeFormat;
use xz_core::options::lzma1::Lzma1Options;
use xz_core::options::{
    Compression, FilterConfig, FilterOptions, FilterType, IntegrityCheck, LzmaOptions,
};
use xz_core::{sanitize_threads, Threading};

use crate::error::{DiagnosticCause, Error, Result};

const LZMA_DICT_MIB: u32 = 1 << 20;

/// Collects CLI compression settings and produces final [`CompressionOptions`].
#[derive(Debug, Clone)]
pub struct Builder {
    format: EncodeFormat,
    level: Compression,
    check: IntegrityCheck,
    pub(crate) threads: Threading,
    block_size: Option<NonZeroU64>,
    timeout: Option<Duration>,
    pub(crate) filters: Vec<FilterConfig>,
    pub(crate) lzma1: Option<Lzma1Options>,
    memory_limit: Option<u64>,
    no_adjust: bool,
}

impl Builder {
    pub fn new(format: EncodeFormat, level: Compression, check: IntegrityCheck) -> Self {
        Self {
            format,
            level,
            check,
            threads: Threading::Auto,
            block_size: None,
            timeout: None,
            filters: Vec::new(),
            lzma1: None,
            memory_limit: None,
            no_adjust: false,
        }
    }

    #[must_use]
    pub fn with_threads(mut self, threads: Threading) -> Self {
        self.threads = threads;
        self
    }

    #[must_use]
    pub fn with_lzma1(mut self, lzma1: Option<Lzma1Options>) -> Self {
        self.lzma1 = lzma1;
        self
    }

    #[must_use]
    pub fn with_filters(mut self, filters: Vec<FilterConfig>) -> Self {
        self.filters = filters;
        self
    }

    #[must_use]
    pub fn with_memory_limit(mut self, limit: Option<u64>) -> Self {
        self.memory_limit = limit;
        self
    }

    #[must_use]
    pub fn with_no_adjust(mut self, no_adjust: bool) -> Self {
        self.no_adjust = no_adjust;
        self
    }

    pub fn build(self) -> Result<CompressionOptions> {
        let Some(limit) = self.memory_limit.filter(|value| *value > 0) else {
            return Ok(into_options(&self, &Resolved::from_builder(&self)));
        };

        let mut resolved = Resolved::from_builder(&self);

        if self.format == EncodeFormat::Raw {
            return check_raw_format_limit(&self, &resolved, limit);
        }

        reduce_mt_threads_for_limit(&self, &mut resolved, limit, self.no_adjust)?;
        ensure_lzma1_for_lzma(&self, &mut resolved)?;

        if estimate_memusage(&self, &resolved)? <= limit {
            return Ok(into_options(&self, &resolved));
        }

        if self.no_adjust {
            return Err(memlimit_too_low(estimate_memusage(&self, &resolved)?));
        }

        if self.format == EncodeFormat::Lzma {
            adjust_lzma1_dict_for_limit(&self, &mut resolved, limit)?;
        } else {
            materialize_preset_filters(&self, &mut resolved)?;
            adjust_lzma_dict_for_limit(&self, &mut resolved, limit)?;
        }

        Ok(into_options(&self, &resolved))
    }
}

#[derive(Debug, Clone)]
struct Resolved {
    threads: Threading,
    xz_mt_encoder: bool,
    filters: Vec<FilterConfig>,
    lzma1: Option<Lzma1Options>,
}

impl Resolved {
    fn from_builder(builder: &Builder) -> Self {
        Self {
            threads: builder.threads,
            xz_mt_encoder: initial_xz_mt_encoding(builder),
            filters: builder.filters.clone(),
            lzma1: builder.lzma1.clone(),
        }
    }
}

fn into_options(builder: &Builder, resolved: &Resolved) -> CompressionOptions {
    let mut options = CompressionOptions::default()
        .with_format(builder.format)
        .with_level(builder.level)
        .with_check(builder.check)
        .with_threads(resolved.threads)
        .with_xz_mt_encoder(resolved.xz_mt_encoder);

    if let Some(block_size) = builder.block_size {
        options = options.with_block_size(Some(block_size));
    }
    if let Some(timeout) = builder.timeout {
        options = options.with_timeout(Some(timeout));
    }
    if !resolved.filters.is_empty() {
        options = options.with_filters(resolved.filters.clone());
    }
    if resolved.lzma1.is_some() {
        options = options.with_lzma1_options(resolved.lzma1.clone());
    }

    options
}

fn initial_xz_mt_encoding(builder: &Builder) -> bool {
    if builder.format != EncodeFormat::Xz {
        return false;
    }

    match builder.threads {
        Threading::Auto => true,
        Threading::Exact(0) => true,
        Threading::Exact(threads) => threads > 1,
    }
}

fn check_raw_format_limit(
    builder: &Builder,
    resolved: &Resolved,
    limit: u64,
) -> Result<CompressionOptions> {
    let memory_usage = estimate_memusage(builder, resolved)?;
    if memory_usage > limit {
        return Err(memlimit_too_low(memory_usage));
    }
    Ok(into_options(builder, resolved))
}

fn reduce_mt_threads_for_limit(
    builder: &Builder,
    resolved: &mut Resolved,
    limit: u64,
    no_adjust: bool,
) -> Result<()> {
    if builder.format != EncodeFormat::Xz || !resolved.xz_mt_encoder {
        return Ok(());
    }

    let initial_threads = sanitize_threads(resolved.threads).map_err(map_thread_error)?;
    let mut threads = initial_threads;

    while threads > 1 {
        threads -= 1;
        resolved.threads = Threading::Exact(threads);
        if estimate_memusage(builder, resolved)? <= limit {
            return Ok(());
        }
    }

    if no_adjust {
        return Err(memlimit_too_low(estimate_memusage(builder, resolved)?));
    }

    resolved.xz_mt_encoder = false;
    resolved.threads = Threading::Exact(1);
    materialize_preset_filters(builder, resolved)?;
    Ok(())
}

fn ensure_lzma1_for_lzma(builder: &Builder, resolved: &mut Resolved) -> Result<()> {
    if builder.format != EncodeFormat::Lzma || resolved.lzma1.is_some() {
        return Ok(());
    }

    resolved.lzma1 = Some(
        Lzma1Options::from_preset(builder.level).map_err(|err| map_core_error(err.into()))?,
    );
    Ok(())
}

fn materialize_preset_filters(builder: &Builder, resolved: &mut Resolved) -> Result<()> {
    if !resolved.filters.is_empty() {
        return Ok(());
    }

    let lzma1 = Lzma1Options::from_preset(builder.level).map_err(|err| map_core_error(err.into()))?;
    let lzma = LzmaOptions::from(&lzma1);
    let filter_type = if builder.format == EncodeFormat::Lzma {
        FilterType::Lzma1
    } else {
        FilterType::Lzma2
    };

    resolved.filters = vec![FilterConfig {
        filter_type,
        options: Some(FilterOptions::Lzma(lzma)),
    }];
    Ok(())
}

fn adjust_lzma1_dict_for_limit(
    builder: &Builder,
    resolved: &mut Resolved,
    limit: u64,
) -> Result<()> {
    let Some(base) = resolved.lzma1.clone() else {
        return Err(memlimit_too_low(u64::MAX));
    };

    let mut dict = base.dict_size() & !(LZMA_DICT_MIB - 1);

    loop {
        if dict < LZMA_DICT_MIB {
            return Err(memlimit_too_low(
                estimate_memusage(builder, resolved).unwrap_or(u64::MAX),
            ));
        }

        resolved.lzma1 = Some(base.clone().with_dict_size(dict));
        let usage = estimate_memusage(builder, resolved)?;
        if usage <= limit {
            return Ok(());
        }
        if dict <= LZMA_DICT_MIB {
            return Err(memlimit_too_low(usage));
        }

        dict -= LZMA_DICT_MIB;
    }
}

fn adjust_lzma_dict_for_limit(
    builder: &Builder,
    resolved: &mut Resolved,
    limit: u64,
) -> Result<()> {
    let Some(index) = find_lzma_filter_index(&resolved.filters) else {
        return Err(memlimit_too_low(u64::MAX));
    };

    {
        let Some(FilterOptions::Lzma(lzma)) = resolved.filters[index].options.as_mut() else {
            return Err(memlimit_too_low(u64::MAX));
        };
        lzma.dict_size &= !(LZMA_DICT_MIB - 1);
    }

    loop {
        let dict_size = match resolved.filters[index].options.as_ref() {
            Some(FilterOptions::Lzma(lzma)) => lzma.dict_size,
            _ => return Err(memlimit_too_low(u64::MAX)),
        };

        if dict_size < LZMA_DICT_MIB {
            return Err(memlimit_too_low(
                estimate_memusage(builder, resolved).unwrap_or(u64::MAX),
            ));
        }

        let usage = estimate_memusage(builder, resolved)?;
        if usage <= limit {
            return Ok(());
        }
        if dict_size <= LZMA_DICT_MIB {
            return Err(memlimit_too_low(usage));
        }

        if let Some(FilterOptions::Lzma(lzma)) = resolved.filters[index].options.as_mut() {
            lzma.dict_size -= LZMA_DICT_MIB;
        }
    }
}

fn find_lzma_filter_index(filters: &[FilterConfig]) -> Option<usize> {
    filters.iter().enumerate().rev().find_map(|(index, filter)| {
        if matches!(
            filter.filter_type,
            FilterType::Lzma1 | FilterType::Lzma1Ext | FilterType::Lzma2
        ) && matches!(filter.options, Some(FilterOptions::Lzma(_)))
        {
            Some(index)
        } else {
            None
        }
    })
}

fn estimate_memusage(builder: &Builder, resolved: &Resolved) -> Result<u64> {
    if matches!(builder.format, EncodeFormat::Lzma | EncodeFormat::Raw) {
        let lzma1 = resolved.lzma1.as_ref().ok_or_else(|| {
            DiagnosticCause::from(Error::InvalidOption {
                message: "internal error: .lzma/.raw compression requires LZMA1 options".into(),
            })
        })?;
        let prepared =
            lzma_safe::encoder::options::prepare_lzma1_filters(lzma1, FilterType::Lzma1);
        return normalize_memusage(raw_encoder_memusage(&prepared));
    }

    if builder.format == EncodeFormat::Xz && resolved.xz_mt_encoder {
        let threads = sanitize_threads(resolved.threads)
            .map_err(map_thread_error)?
            .max(1);
        let mut mt = EncoderMtOptions::default()
            .with_level(builder.level)
            .with_check(builder.check)
            .with_threads(threads);

        if let Some(block) = builder.block_size {
            mt = mt.with_block_size(block.get());
        }

        if !resolved.filters.is_empty() {
            mt = mt.with_filters(resolved.filters.clone());
        }

        return normalize_memusage(mt_encoder_memusage(&mt));
    }

    if !resolved.filters.is_empty() {
        return normalize_memusage(filters_encoder_memusage(&resolved.filters));
    }

    normalize_memusage(easy_encoder_memusage(builder.level))
}

fn normalize_memusage(usage: u64) -> Result<u64> {
    if usage == u64::MAX {
        Err(DiagnosticCause::from(Error::InvalidOption {
            message: "unsupported filter chain or filter options".into(),
        }))
    } else {
        Ok(usage)
    }
}

fn memlimit_too_low(memory_usage: u64) -> DiagnosticCause {
    DiagnosticCause::from(Error::InvalidOption {
        message: format!(
            "memory usage limit is too low for the given filter setup (at least {memory_usage} bytes required)"
        ),
    })
}

fn map_thread_error(err: xz_core::Error) -> DiagnosticCause {
    match err {
        xz_core::Error::InvalidThreadCount { requested, maximum: _ } => {
            DiagnosticCause::from(Error::InvalidThreadCount {
                count: usize::try_from(requested).unwrap_or(usize::MAX),
            })
        }
        xz_core::Error::InvalidOption(message) => {
            DiagnosticCause::from(Error::InvalidOption { message })
        }
        other => DiagnosticCause::from(Error::InvalidOption {
            message: other.to_string(),
        }),
    }
}

fn map_core_error(err: xz_core::Error) -> DiagnosticCause {
    match err {
        xz_core::Error::InvalidOption(message) => {
            DiagnosticCause::from(Error::InvalidOption { message })
        }
        xz_core::Error::Backend(backend) => DiagnosticCause::from(Error::InvalidOption {
            message: backend.xz_message().to_string(),
        }),
        other => DiagnosticCause::from(Error::InvalidOption {
            message: other.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use xz_core::config::EncodeFormat;
    use xz_core::options::Compression;
    use xz_core::pipeline::compress;

    use super::*;

    #[test]
    fn generic_one_megabyte_limit_fails_for_default_preset() {
        let err = Builder::new(EncodeFormat::Xz, Compression::Level6, IntegrityCheck::Crc64)
            .with_threads(Threading::Exact(1))
            .with_memory_limit(Some(1024 * 1024))
            .build()
            .expect_err("level 6 preset cannot fit into 1 MiB limit");
        assert!(matches!(
            err.as_error(),
            Some(Error::InvalidOption { .. })
        ));
        assert!(
            err.to_string().contains("memory usage limit is too low"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn no_adjust_fails_when_preset_exceeds_limit() {
        let err = Builder::new(EncodeFormat::Xz, Compression::Level9, IntegrityCheck::Crc64)
            .with_threads(Threading::Exact(1))
            .with_memory_limit(Some(1024))
            .with_no_adjust(true)
            .build()
            .expect_err("level 9 preset should exceed 1 KiB limit");
        assert!(matches!(
            err.as_error(),
            Some(Error::InvalidOption { .. })
        ));
    }

    #[test]
    fn adjusts_dict_size_to_meet_explicit_limit() {
        let options = Builder::new(EncodeFormat::Xz, Compression::Level4, IntegrityCheck::Crc64)
            .with_threads(Threading::Exact(1))
            .with_memory_limit(Some(40 * 1024 * 1024))
            .build()
            .expect("dict size should be reduced to fit limit");

        let input = b"abc\n".repeat(128);
        let mut output = Vec::new();
        compress(&mut Cursor::new(input), &mut output, &options)
            .expect("adjusted settings should compress successfully");
        assert!(!output.is_empty());
    }

    #[test]
    fn reduces_thread_count_before_adjusting_dict() {
        let max_threads = sanitize_threads(Threading::Auto).expect("threads");
        if max_threads <= 1 {
            return;
        }

        let options = Builder::new(EncodeFormat::Xz, Compression::Level6, IntegrityCheck::Crc64)
            .with_threads(Threading::Exact(max_threads))
            .with_memory_limit(Some(64 * 1024 * 1024))
            .build()
            .expect("thread count should be reduced to fit limit");

        let probe = Builder::new(EncodeFormat::Xz, Compression::Level6, IntegrityCheck::Crc64)
            .with_threads(Threading::Exact(max_threads));
        let mut resolved = Resolved::from_builder(&probe);
        reduce_mt_threads_for_limit(&probe, &mut resolved, 64 * 1024 * 1024, false).unwrap();
        let Threading::Exact(threads) = resolved.threads else {
            panic!("expected explicit thread count after adjustment");
        };
        assert!(threads < max_threads);
        assert!(threads >= 1);
        let _ = options;
    }

    #[test]
    fn compresses_with_memlimit_compress_and_no_adjust() {
        let input = b"abc\n".repeat(12_345);
        let mut output = Vec::new();

        let options = Builder::new(EncodeFormat::Xz, Compression::Level4, IntegrityCheck::Crc64)
            .with_threads(Threading::Exact(1))
            .with_memory_limit(Some(48 * 1024 * 1024))
            .with_no_adjust(true)
            .build()
            .expect("settings should fit within upstream interop limits");

        compress(&mut Cursor::new(input), &mut output, &options)
            .expect("compression should succeed within upstream interop limits");
        assert!(!output.is_empty());
    }

    #[test]
    fn raw_format_rejects_limit_without_adjustment() {
        let err = Builder::new(EncodeFormat::Raw, Compression::Level9, IntegrityCheck::Crc64)
            .with_lzma1(Some(
                Lzma1Options::from_preset(Compression::Level9).expect("preset"),
            ))
            .with_memory_limit(Some(1024))
            .build()
            .expect_err("raw streams never auto-adjust");
        assert!(matches!(
            err.as_error(),
            Some(Error::InvalidOption { .. })
        ));
    }
}
