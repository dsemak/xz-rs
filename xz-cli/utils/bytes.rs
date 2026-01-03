//! Byte-size formatting helpers.

/// Format a byte count like upstream `xz -l`.
///
/// Uses `KiB` for values >= 1024 bytes and `MiB` for values >= 1 MiB.
pub(crate) fn format_list_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;

    if bytes >= MIB {
        let tenths = bytes.saturating_mul(10) / MIB;
        let whole = tenths / 10;
        let frac = tenths % 10;
        format!("{whole}.{frac} MiB")
    } else if bytes >= KIB {
        let tenths = bytes.saturating_mul(10) / KIB;
        let whole = tenths / 10;
        let frac = tenths % 10;
        format!("{whole}.{frac} KiB")
    } else {
        format!("{bytes} B")
    }
}

/// Format a size for the verbose output, optionally appending raw bytes.
pub(crate) fn format_list_size_with_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format_list_size(bytes)
    } else {
        format!("{} ({bytes} B)", format_list_size(bytes))
    }
}
