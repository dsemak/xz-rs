//! Helper types describing filter chains passed to liblzma.

/// Single element of an encoder filter chain.
#[derive(Debug, Clone)]
pub struct FilterConfig {
    /// Which filter to apply.
    pub filter_type: FilterType,

    /// Optional filter-specific configuration.
    pub options: Option<FilterOptions>,
}

/// Filter-specific configuration payloads.
#[derive(Debug, Clone)]
pub enum FilterOptions {
    /// Options for LZMA1/LZMA2 filters.
    Lzma(LzmaOptions),

    /// Options for BCJ filters.
    Bcj(BcjOptions),

    /// Options for the delta filter.
    Delta(DeltaOptions),
}

/// Filter identifiers mirroring the constants in liblzma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum FilterType {
    /// `LZMA_FILTER_LZMA1`.
    Lzma1 = 0x4000_0000_0000_0001,

    /// `LZMA_FILTER_LZMA1EXT`.
    Lzma1Ext = 0x4000_0000_0000_0002,

    /// `LZMA_FILTER_LZMA2`.
    Lzma2 = 0x21,

    /// `LZMA_FILTER_X86`.
    X86 = 0x04,

    /// `LZMA_FILTER_POWERPC`.
    PowerPc = 0x05,

    /// `LZMA_FILTER_IA64`.
    Ia64 = 0x06,

    /// `LZMA_FILTER_ARM`.
    Arm = 0x07,

    /// `LZMA_FILTER_ARMTHUMB`.
    ArmThumb = 0x08,

    /// `LZMA_FILTER_ARM64`.
    Arm64 = 0x0A,

    /// `LZMA_FILTER_SPARC`.
    Sparc = 0x09,

    /// `LZMA_FILTER_RISCV`.
    RiscV = 0x0B,

    /// `LZMA_FILTER_DELTA`.
    Delta = 0x03,
}

impl FilterType {
    /// Returns the numeric filter ID as expected by liblzma.
    fn to_lzma_id(self) -> u64 {
        self as u64
    }
}

/// Compression mode offered by liblzma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CompressionMode {
    /// Faster compression, lower ratio (`LZMA_MODE_FAST`).
    Fast = 1,
    /// Better ratio, slower (`LZMA_MODE_NORMAL`).
    Normal = 2,
}

impl From<CompressionMode> for liblzma_sys::lzma_mode {
    fn from(mode: CompressionMode) -> Self {
        mode as u32
    }
}

/// Match finder algorithms supported by liblzma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MatchFinder {
    /// Hash chain with 2-/3-byte hashing (`LZMA_MF_HC3`).
    Hc3 = 0x03,
    /// Hash chain with 2-/3-/4-byte hashing (`LZMA_MF_HC4`).
    Hc4 = 0x04,
    /// Binary tree with 2-byte hashing (`LZMA_MF_BT2`).
    Bt2 = 0x12,
    /// Binary tree with 2-/3-byte hashing (`LZMA_MF_BT3`).
    Bt3 = 0x13,
    /// Binary tree with 2-/3-/4-byte hashing (`LZMA_MF_BT4`).
    Bt4 = 0x14,
}

impl From<MatchFinder> for liblzma_sys::lzma_match_finder {
    fn from(mf: MatchFinder) -> Self {
        mf as u32
    }
}

/// Parameters for the LZMA1/LZMA2 filters exposed via liblzma.
#[derive(Debug, Clone)]
pub struct LzmaOptions {
    /// Dictionary size in bytes.
    pub dict_size: u32,

    /// Literal context bits.
    pub lc: u32,

    /// Literal position bits.
    pub lp: u32,

    /// Position bits used for match distances.
    pub pb: u32,

    /// Compression mode (`Fast` or `Normal`).
    pub mode: CompressionMode,

    /// Upper bound for search length when looking for matches.
    pub nice_len: u32,

    /// Match finder algorithm.
    pub mf: MatchFinder,

    /// Maximum search depth; 0 lets liblzma decide.
    pub depth: u32,

    /// Optional preset dictionary bytes.
    pub preset_dict: Option<Vec<u8>>,

    /// Extra flags used only by the LZMA1EXT format.
    pub ext_flags: u32,

    /// Uncompressed size low bits (LZMA1EXT).
    pub ext_size_low: u32,

    /// Uncompressed size high bits (LZMA1EXT).
    pub ext_size_high: u32,
}

impl LzmaOptions {
    /// Default dictionary size (8 MiB).
    const LZMA_DICT_SIZE_DEFAULT: u32 = 1 << 23;
    /// Default number of literal context bits.
    const LZMA_LC_DEFAULT: u32 = 3;
    /// Default number of literal position bits.
    const LZMA_LP_DEFAULT: u32 = 0;
    /// Default number of position bits.
    const LZMA_PB_DEFAULT: u32 = 2;
}

impl Default for LzmaOptions {
    fn default() -> Self {
        Self {
            dict_size: Self::LZMA_DICT_SIZE_DEFAULT,
            lc: Self::LZMA_LC_DEFAULT,
            lp: Self::LZMA_LP_DEFAULT,
            pb: Self::LZMA_PB_DEFAULT,
            mode: CompressionMode::Normal,
            nice_len: 64,
            mf: MatchFinder::Hc4,
            depth: 0, // 0 means automatic depth selection by liblzma
            preset_dict: None,
            ext_flags: 0,
            ext_size_low: 0,
            ext_size_high: 0,
        }
    }
}

/// Options for BCJ (Branch/Call/Jump) filters.
#[derive(Debug, Clone, Default)]
pub struct BcjOptions {
    /// Start offset added to converted branch targets.
    pub start_offset: u32,
}

/// Options for the delta pre-processing filter.
#[derive(Debug, Clone)]
pub struct DeltaOptions {
    /// Distance in bytes to look back when computing the delta.
    pub distance: u32,
}

impl Default for DeltaOptions {
    fn default() -> Self {
        Self { distance: 1 }
    }
}

/// Keeps allocated filter options alive for the duration of an FFI call.
pub enum OwnedFilterOptions {
    /// LZMA1/2 options and optional preset dictionary.
    Lzma {
        opts: Box<liblzma_sys::lzma_options_lzma>,
        dict: Option<Box<[u8]>>,
    },
    /// BCJ filter options.
    Bcj(Box<liblzma_sys::lzma_options_bcj>),
    /// Delta filter options.
    Delta(Box<liblzma_sys::lzma_options_delta>),
    /// No options (for filters that do not require them).
    None,
}

/// Prepared filter chain plus the owned option storage.
pub struct RawFilters {
    /// The filter chain as expected by liblzma (terminated by `LZMA_VLI_UNKNOWN`).
    pub filters: Vec<liblzma_sys::lzma_filter>,
    /// Owned option buffers to ensure pointers remain valid.
    pub owned: Vec<OwnedFilterOptions>,
}

impl RawFilters {
    /// Return a pointer to the filter chain expected by liblzma.
    ///
    /// # Safety
    ///
    /// Pointer is valid while `self` is alive.
    pub fn as_ptr(&self) -> *const liblzma_sys::lzma_filter {
        if self.filters.is_empty() {
            std::ptr::null()
        } else {
            self.filters.as_ptr()
        }
    }
}

/// Creates LZMA filter options and returns the filter entry with owned options.
///
/// # Parameters
///
/// - `filter_type`: Type of LZMA filter (`LZMA1`, `LZMA1Ext`, or `LZMA2`).
/// - `user_options`: Optional user-provided LZMA options.
///
/// # Returns
///
/// A tuple containing the `lzma_filter` entry and the owned options.
fn create_lzma_filter(
    filter_type: FilterType,
    user_options: Option<&LzmaOptions>,
) -> (liblzma_sys::lzma_filter, OwnedFilterOptions) {
    use std::os::raw::c_void;

    // Create default LZMA options structure.
    let mut opts = Box::new(liblzma_sys::lzma_options_lzma {
        dict_size: LzmaOptions::LZMA_DICT_SIZE_DEFAULT,
        preset_dict: std::ptr::null(),
        preset_dict_size: 0,
        lc: LzmaOptions::LZMA_LC_DEFAULT,
        lp: LzmaOptions::LZMA_LP_DEFAULT,
        pb: LzmaOptions::LZMA_PB_DEFAULT,
        mode: liblzma_sys::lzma_mode_LZMA_MODE_NORMAL,
        nice_len: 64,
        mf: liblzma_sys::lzma_match_finder_LZMA_MF_HC4,
        depth: 0,
        ext_flags: 0,
        ext_size_low: 0,
        ext_size_high: 0,
        reserved_int4: 0,
        reserved_int5: 0,
        reserved_int6: 0,
        reserved_int7: 0,
        reserved_int8: 0,
        reserved_enum1: 0,
        reserved_enum2: 0,
        reserved_enum3: 0,
        reserved_enum4: 0,
        reserved_ptr1: std::ptr::null_mut(),
        reserved_ptr2: std::ptr::null_mut(),
    });

    // Apply user options if provided and handle preset dictionary.
    let dict_owned = if let Some(lo) = user_options {
        // SAFETY: All values are copied into the C struct; liblzma will validate ranges.
        opts.dict_size = lo.dict_size;
        opts.lc = lo.lc;
        opts.lp = lo.lp;
        opts.pb = lo.pb;
        opts.mode = lo.mode.into();
        opts.nice_len = lo.nice_len;
        opts.mf = lo.mf.into();
        opts.depth = lo.depth;
        opts.ext_flags = lo.ext_flags;
        opts.ext_size_low = lo.ext_size_low;
        opts.ext_size_high = lo.ext_size_high;

        // Handle preset dictionary if provided.
        let dict = lo
            .preset_dict
            .as_ref()
            .map(|v| v.clone().into_boxed_slice());
        if let Some(ref d) = dict {
            opts.preset_dict = d.as_ptr();
            #[allow(clippy::cast_possible_truncation)]
            {
                opts.preset_dict_size = d.len() as u32;
            }
        }
        dict
    } else {
        None
    };

    let owned = OwnedFilterOptions::Lzma {
        opts,
        dict: dict_owned,
    };

    // Get pointer to the options struct.
    let opt_ptr = match &owned {
        OwnedFilterOptions::Lzma { opts, .. } => std::ptr::addr_of!(**opts) as *mut c_void,
        _ => unreachable!(),
    };

    let filter = liblzma_sys::lzma_filter {
        id: filter_type.to_lzma_id(),
        options: opt_ptr,
    };

    (filter, owned)
}

/// Creates Delta filter options and returns the filter entry with owned options.
///
/// # Parameters
///
/// - `user_options`: Optional user-provided Delta options.
///
/// # Returns
///
/// A tuple containing the `lzma_filter` entry and the owned options.
fn create_delta_filter(
    user_options: Option<&DeltaOptions>,
) -> (liblzma_sys::lzma_filter, OwnedFilterOptions) {
    use std::os::raw::c_void;

    // Create default Delta options structure.
    let mut opts = Box::new(liblzma_sys::lzma_options_delta {
        type_: liblzma_sys::lzma_delta_type_LZMA_DELTA_TYPE_BYTE,
        dist: DeltaOptions::default().distance,
        reserved_int1: 0,
        reserved_int2: 0,
        reserved_int3: 0,
        reserved_int4: 0,
        reserved_ptr1: std::ptr::null_mut(),
        reserved_ptr2: std::ptr::null_mut(),
    });

    // Apply user options if provided.
    if let Some(do_) = user_options {
        opts.dist = do_.distance;
    }

    let owned = OwnedFilterOptions::Delta(opts);

    // Get pointer to the options struct.
    let opt_ptr = match &owned {
        OwnedFilterOptions::Delta(opts) => std::ptr::addr_of!(**opts) as *mut c_void,
        _ => unreachable!(),
    };

    let filter = liblzma_sys::lzma_filter {
        id: FilterType::Delta.to_lzma_id(),
        options: opt_ptr,
    };

    (filter, owned)
}

/// Creates BCJ filter options and returns the filter entry with owned options.
///
/// # Parameters
///
/// - `filter_type`: Type of BCJ filter.
/// - `user_options`: Optional user-provided BCJ options.
///
/// # Returns
///
/// A tuple containing the `lzma_filter` entry and the owned options.
fn create_bcj_filter(
    filter_type: FilterType,
    user_options: Option<&BcjOptions>,
) -> (liblzma_sys::lzma_filter, OwnedFilterOptions) {
    use std::os::raw::c_void;

    // Create default BCJ options structure.
    let mut opts = Box::new(liblzma_sys::lzma_options_bcj {
        start_offset: BcjOptions::default().start_offset,
    });

    // Apply user options if provided.
    if let Some(bo) = user_options {
        opts.start_offset = bo.start_offset;
    }

    let owned = OwnedFilterOptions::Bcj(opts);

    // Get pointer to the options struct.
    let opt_ptr = match &owned {
        OwnedFilterOptions::Bcj(opts) => std::ptr::addr_of!(**opts) as *mut c_void,
        _ => unreachable!(),
    };

    let filter = liblzma_sys::lzma_filter {
        id: filter_type.to_lzma_id(),
        options: opt_ptr,
    };

    (filter, owned)
}

/// Prepares a liblzma filter chain and collects owned option buffers.
///
/// This function builds a filter chain (terminated by `LZMA_VLI_UNKNOWN`)
/// and allocates/owns all option buffers so their pointers remain valid
/// during the FFI call to liblzma. Each filter in `configs` is converted
/// to a `liblzma_sys::lzma_filter` entry with the appropriate options.
///
/// # Parameters
///
/// - `configs`: Slice of filter configurations to build the chain.
///
/// # Returns
///
/// - `RawFilters`: Contains the filter chain and owned option buffers.
///
/// # Safety
///
/// The returned pointers are valid as long as the returned `RawFilters` is alive.
pub(crate) fn prepare_filters(configs: &[FilterConfig]) -> RawFilters {
    // Preallocate space for the filter chain and owned option buffers.
    let mut filters = Vec::with_capacity(configs.len() + 1);
    let mut owned = Vec::with_capacity(configs.len());

    for cfg in configs {
        let (filter, owned_opts) = match (cfg.filter_type, &cfg.options) {
            (FilterType::Lzma1 | FilterType::Lzma1Ext | FilterType::Lzma2, maybe) => {
                let lzma_opts = maybe.as_ref().and_then(|o| match o {
                    FilterOptions::Lzma(lo) => Some(lo),
                    _ => None,
                });
                create_lzma_filter(cfg.filter_type, lzma_opts)
            }

            (FilterType::Delta, maybe) => {
                let delta_opts = maybe.as_ref().and_then(|o| match o {
                    FilterOptions::Delta(do_) => Some(do_),
                    _ => None,
                });
                create_delta_filter(delta_opts)
            }

            (
                FilterType::X86
                | FilterType::PowerPc
                | FilterType::Ia64
                | FilterType::Arm
                | FilterType::ArmThumb
                | FilterType::Arm64
                | FilterType::Sparc
                | FilterType::RiscV,
                maybe,
            ) => {
                let bcj_opts = maybe.as_ref().and_then(|o| match o {
                    FilterOptions::Bcj(bo) => Some(bo),
                    _ => None,
                });
                create_bcj_filter(cfg.filter_type, bcj_opts)
            }
        };

        filters.push(filter);
        owned.push(owned_opts);
    }

    // Terminate the filter chain with LZMA_VLI_UNKNOWN (0) as required by liblzma.
    filters.push(liblzma_sys::lzma_filter {
        id: 0,
        options: std::ptr::null_mut(),
    });

    RawFilters { filters, owned }
}
