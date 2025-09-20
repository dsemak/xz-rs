//! Encoder configuration helpers shared by the safe wrappers.

mod check;
mod filter;
mod present;

pub use check::IntegrityCheck;
pub use filter::{FilterConfig, FilterOptions, FilterType, OwnedFilterOptions, RawFilters};
pub use present::Compression;

/// Options forwarded to `lzma_stream_encoder_mt`.
pub struct Options {
    /// Compression preset; ignored when `filters` is non-empty.
    pub level: Compression,

    /// Integrity check to embed in the output stream.
    pub check: IntegrityCheck,

    /// Number of worker threads (0/1 means single-threaded).
    pub threads: u32,

    /// Maximum size of a block in bytes; `0` uses liblzma defaults.
    pub block_size: u64,

    /// Timeout in milliseconds; `0` disables timeouts.
    pub timeout: u32,

    /// Optional filter chain; when present the preset is ignored.
    pub filters: Vec<FilterConfig>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            level: Compression::Level6,
            check: IntegrityCheck::Crc32,
            threads: 0,
            block_size: 0, // Automatic
            timeout: 0,    // No timeout
            filters: Vec::new(),
        }
    }
}

impl std::fmt::Debug for Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncoderOptions")
            .field("level", &self.level)
            .field("check", &self.check)
            .field("threads", &self.threads)
            .field("block_size", &self.block_size)
            .field("timeout", &self.timeout)
            .field("filters", &self.filters)
            .finish()
    }
}

impl Options {
    /// Set the compression preset.
    #[must_use]
    pub fn with_level(mut self, level: Compression) -> Self {
        self.level = level;
        self
    }

    /// Set the integrity check.
    #[must_use]
    pub fn with_check(mut self, check: IntegrityCheck) -> Self {
        self.check = check;
        self
    }

    /// Set the number of worker threads.
    #[must_use]
    pub fn with_threads(mut self, threads: u32) -> Self {
        self.threads = threads;
        self
    }

    /// Set the block size in bytes.
    #[must_use]
    pub fn with_block_size(mut self, block_size: u64) -> Self {
        self.block_size = block_size;
        self
    }

    /// Set the timeout in milliseconds.
    #[must_use]
    pub fn with_timeout(mut self, timeout: u32) -> Self {
        self.timeout = timeout;
        self
    }

    /// Replace the filter chain.
    #[must_use]
    pub fn with_filters(mut self, filters: Vec<FilterConfig>) -> Self {
        self.filters = filters;
        self
    }

    /// Append a filter to the existing chain.
    #[must_use]
    pub fn with_filter(mut self, filter: FilterConfig) -> Self {
        self.filters.push(filter);
        self
    }

    /// Convert to the raw `lzma_mt` structure and keep filter buffers alive if needed.
    pub(crate) fn to_lzma_options(&self) -> (liblzma_sys::lzma_mt, Option<filter::RawFilters>) {
        // SAFETY: lzma_mt is a POD struct; zeroed then filled with required fields.
        let mut options = unsafe { std::mem::zeroed::<liblzma_sys::lzma_mt>() };

        // Set only the required fields; leave others at their default zeroed values.
        options.threads = self.threads;
        options.block_size = self.block_size;
        options.timeout = self.timeout;
        options.preset = self.level.to_preset();
        options.check = self.check.into();
        let prepared = if self.filters.is_empty() {
            None
        } else {
            let prepared = filter::prepare_filters(&self.filters);
            options.filters = prepared.as_ptr();
            Some(prepared)
        };

        (options, prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test conversion to `lzma_mt` without filters.
    #[test]
    fn to_lzma_options_without_filters_sets_core_fields() {
        let options = Options::default()
            .with_level(Compression::Level4)
            .with_check(IntegrityCheck::Sha256)
            .with_threads(3)
            .with_block_size(512 * 1024)
            .with_timeout(250);

        let (mt, prepared) = options.to_lzma_options();

        assert_eq!(mt.threads, 3);
        assert_eq!(mt.block_size, 512 * 1024);
        assert_eq!(mt.timeout, 250);
        assert_eq!(mt.preset, Compression::Level4.to_preset());
        assert_eq!(mt.check, IntegrityCheck::Sha256.into());
        assert!(mt.filters.is_null());
        assert!(prepared.is_none());
    }

    /// Test conversion to `lzma_mt` with filter chain.
    #[test]
    fn to_lzma_options_with_filters_builds_chain() {
        let options = Options::default().with_filters(vec![
            FilterConfig {
                filter_type: FilterType::Lzma2,
                options: None,
            },
            FilterConfig {
                filter_type: FilterType::Delta,
                options: None,
            },
        ]);

        let (mt, prepared) = options.to_lzma_options();
        let raw = prepared.expect("expected prepared filters");

        assert!(!mt.filters.is_null());
        assert_eq!(raw.filters.len(), 3); // two filters + terminator
        assert_eq!(raw.filters[0].id, FilterType::Lzma2 as u64);
        assert_eq!(raw.filters[1].id, FilterType::Delta as u64);
        assert_eq!(raw.filters[2].id, 0);
        assert!(!raw.filters[0].options.is_null());
        assert!(!raw.filters[1].options.is_null());
    }
}
