//! Byte-size formatting helpers.

/// Format a byte count like upstream `xz -l`.
///
/// Uses `KiB` for values >= 1024 bytes and `MiB` for values >= 1 MiB.
pub(crate) fn format_list_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;

    let bytes_f = bytes as f64;
    if bytes_f >= MIB {
        format!("{:.1} MiB", bytes_f / MIB)
    } else if bytes_f >= KIB {
        format!("{:.1} KiB", bytes_f / KIB)
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
