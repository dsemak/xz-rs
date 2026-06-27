use std::time::Duration;

pub(crate) const DEFAULT_INPUT_BUFFER: usize = 64 * 1024;
pub(crate) const DEFAULT_OUTPUT_BUFFER: usize = 64 * 1024;

pub(crate) fn duration_to_timeout(duration: Duration) -> u32 {
    duration.as_millis().try_into().unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_conversion_handles_normal_durations() {
        assert_eq!(duration_to_timeout(Duration::from_millis(0)), 0);
        assert_eq!(duration_to_timeout(Duration::from_millis(1)), 1);
        assert_eq!(duration_to_timeout(Duration::from_millis(1000)), 1000);
        assert_eq!(duration_to_timeout(Duration::from_secs(1)), 1000);
        assert_eq!(duration_to_timeout(Duration::from_secs(60)), 60000);
    }

    #[test]
    fn timeout_conversion_saturates() {
        let overflowing = Duration::from_millis(u64::from(u32::MAX) + 5);
        assert_eq!(duration_to_timeout(overflowing), u32::MAX);

        let exact_max = Duration::from_millis(u64::from(u32::MAX));
        assert_eq!(duration_to_timeout(exact_max), u32::MAX);

        let just_under_max = Duration::from_millis(u64::from(u32::MAX) - 1);
        assert_eq!(duration_to_timeout(just_under_max), u32::MAX - 1);
    }

    #[test]
    fn timeout_conversion_edge_cases() {
        let max_safe = Duration::from_millis(u64::from(u32::MAX));
        assert_eq!(duration_to_timeout(max_safe), u32::MAX);

        let huge = Duration::from_secs(u64::MAX);
        assert_eq!(duration_to_timeout(huge), u32::MAX);
    }
}
