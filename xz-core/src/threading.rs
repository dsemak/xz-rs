//! Utilities for configuring safe thread counts for XZ compression/decompression.

use crate::error::{Error, Result};

/// Thread configuration options for compression and decompression operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Threading {
    /// Automatically choose a thread count that keeps a safety margin for the rest of the system.
    ///
    /// This option will detect the number of available CPU cores and reserve some threads
    /// for system processes to prevent resource starvation.
    #[default]
    Auto,
    /// Use an explicit number of worker threads.
    ///
    /// The specified count must not exceed the safe maximum determined by the system.
    /// If 0 is specified, it will be treated as `Auto`.
    Exact(u32),
}

/// Validates and converts a threading configuration to a concrete thread count.
///
/// # Parameters
///
/// * `threads` - The threading configuration to validate and convert
///
/// # Returns
///
/// * `Ok(u32)` - A safe thread count to use
/// * `Err(Error::InvalidThreadCount)` - If the requested thread count exceeds system limits
pub(crate) fn sanitize_threads(threads: Threading) -> Result<u32> {
    let maximum = get_safe_max_threads();
    match threads {
        // Zero threads means "auto-detect"
        Threading::Auto | Threading::Exact(0) => Ok(maximum),
        // Valid explicit thread count
        Threading::Exact(requested) if requested <= maximum => Ok(requested),
        // Thread count exceeds safe limits
        Threading::Exact(requested) => Err(Error::InvalidThreadCount { requested, maximum }),
    }
}

/// Determines the maximum safe number of threads to use for compression/decompression.
///
/// # Returns
///
/// The maximum safe number of threads as a `u32`. If thread detection fails,
/// defaults to 1 thread. If the calculated value exceeds `u32::MAX`, returns `u32::MAX`.
fn get_safe_max_threads() -> u32 {
    // Detect available CPU threads, fallback to 1 if detection fails
    let available_threads_count = match std::thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => 1, // Conservative fallback for systems where detection fails
    };

    // Reserve threads for system processes based on total available threads
    let system_reserve = match available_threads_count {
        1 => 0,     // Single-core: use all available
        2..=4 => 1, // Dual/quad-core: reserve 1 for system
        5..=7 => 2, // Mid-range: reserve 2 for system
        _ => 3,     // High-end: reserve 3 for system
    };

    // Calculate safe thread count, ensuring at least 1 thread is available
    let safe_threads = available_threads_count
        .saturating_sub(system_reserve)
        .max(1); // Always ensure at least 1 thread for compression work

    // Convert to u32, handling potential overflow on exotic architectures
    u32::try_from(safe_threads).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Test that [`sanitize_threads`] respects system limits and handles edge cases correctly.
    fn sanitize_threads_respects_limits() {
        let max = get_safe_max_threads();

        // Auto should always return a valid thread count >= 1
        assert!(matches!(sanitize_threads(Threading::Auto), Ok(n) if n >= 1));
        assert!(matches!(sanitize_threads(Threading::Auto), Ok(n) if n == max));

        // Exact with valid count should return that count
        assert!(matches!(sanitize_threads(Threading::Exact(max)), Ok(n) if n == max));

        // Exact with count exceeding maximum should return error
        assert!(matches!(
            sanitize_threads(Threading::Exact(max + 1)),
            Err(Error::InvalidThreadCount { requested, maximum }) if requested == max + 1 && maximum == max
        ));
    }

    #[test]
    /// Test that zero threads is treated as Auto configuration.
    fn sanitize_threads_zero_means_auto() {
        let max = get_safe_max_threads();
        let result = sanitize_threads(Threading::Exact(0));
        assert!(matches!(result, Ok(n) if n == max));
    }

    #[test]
    /// Test various valid thread counts within limits.
    fn sanitize_threads_valid_counts() {
        let max = get_safe_max_threads();

        // Test thread count of 1 (should always be valid)
        assert!(matches!(sanitize_threads(Threading::Exact(1)), Ok(1)));

        // Test mid-range values if system supports them
        if max >= 2 {
            assert!(matches!(sanitize_threads(Threading::Exact(2)), Ok(2)));
        }
        if max >= 4 {
            assert!(matches!(sanitize_threads(Threading::Exact(4)), Ok(4)));
        }
    }

    #[test]
    /// Test that [`get_safe_max_threads`] returns reasonable values.
    fn get_safe_max_threads_sanity() {
        let max = get_safe_max_threads();

        // Should always return at least 1 thread
        assert!(max >= 1);

        // Should not exceed a reasonable upper bound for typical systems
        // Even on high-end systems, we shouldn't see more than a few hundred threads
        assert!(max <= 1000, "Thread count {max} seems unreasonably high");
    }

    #[test]
    /// Test Threading enum default behavior.
    fn threading_default_is_auto() {
        assert_eq!(Threading::default(), Threading::Auto);
    }

    #[test]
    /// Test Threading enum equality and debug formatting.
    fn threading_traits() {
        let auto1 = Threading::Auto;
        let auto2 = Threading::Auto;
        let exact1 = Threading::Exact(4);
        let exact2 = Threading::Exact(4);
        let exact3 = Threading::Exact(8);

        // Test PartialEq
        assert_eq!(auto1, auto2);
        assert_eq!(exact1, exact2);
        assert_ne!(auto1, exact1);
        assert_ne!(exact1, exact3);

        // Test Debug formatting (should not panic)
        let _debug_auto = format!("{auto1:?}");
        let _debug_exact = format!("{exact1:?}");
    }

    #[test]
    /// Test that [`sanitize_threads`] handles edge cases near u32 limits.
    fn sanitize_threads_boundary_conditions() {
        let max = get_safe_max_threads();

        // Test maximum valid value
        assert!(matches!(sanitize_threads(Threading::Exact(max)), Ok(n) if n == max));

        // Test just over the limit
        if max < u32::MAX {
            assert!(matches!(
                sanitize_threads(Threading::Exact(max + 1)),
                Err(Error::InvalidThreadCount { requested, maximum })
                if requested == max + 1 && maximum == max
            ));
        }

        // Test very large values
        assert!(matches!(
            sanitize_threads(Threading::Exact(u32::MAX)),
            Err(Error::InvalidThreadCount { requested, maximum })
            if requested == u32::MAX && maximum == max
        ));
    }

    #[test]
    /// Test that [`get_safe_max_threads`] produces consistent results.
    fn get_safe_max_threads_consistency() {
        let first_call = get_safe_max_threads();
        let second_call = get_safe_max_threads();

        // Should be deterministic
        assert_eq!(first_call, second_call);
    }
}
