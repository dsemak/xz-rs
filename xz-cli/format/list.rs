//! Formatting helpers for `xz -l` / `xz -l -v`.

use std::io;

use crate::error::{DiagnosticCause, Error, Result};
use crate::utils::{bytes, math};
use xz_core::file_info::{BlockInfo, StreamInfo};

/// Output context for `xz -l` formatting across multiple files.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ListOutputContext {
    /// 1-based index of the current file in the invocation.
    pub file_index: usize,
    /// Total number of files in the invocation.
    pub file_count: usize,
    /// Whether to print the table header before the entry line.
    pub print_header: bool,
}

/// Summary information for one `xz -l` entry.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ListSummary {
    /// Number of streams in the file.
    pub stream_count: u64,
    /// Total number of blocks across all streams.
    pub block_count: u64,
    /// Compressed file size in bytes.
    pub compressed: u64,
    /// Uncompressed data size in bytes.
    pub uncompressed: u64,
    /// Bitmask of integrity checks used across all streams.
    pub checks_mask: u32,
}

/// Convert an XZ "index checks" bitmask into a human-readable check name.
///
/// Returns "None" for no checks, "Mixed" for multiple different checks,
/// or the specific check name (CRC32, CRC64, SHA256) for a single check type.
///
/// # Parameters
///
/// * `checks_mask` - Bitmask where each bit represents a check type
///
/// # Returns
///
/// A static string slice with the check name.
pub(crate) fn format_check_name(checks_mask: u32) -> &'static str {
    if checks_mask == 0 {
        return "None";
    }

    if !checks_mask.is_power_of_two() {
        return "Mixed";
    }

    match checks_mask.trailing_zeros() {
        0 => "None",
        1 => "CRC32",
        4 => "CRC64",
        10 => "SHA256",
        _ => "Unknown",
    }
}

fn write_stdout_line(line: &str) -> Result<()> {
    use std::io::Write;

    let mut out = io::stdout().lock();
    writeln!(out, "{line}").map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    Ok(())
}

/// Print a summary line for multiple files processed with `xz -l`.
///
/// This prints a separator line followed by a totals row matching the format
/// of upstream `xz -l` when processing multiple files.
///
/// # Parameters
///
/// * `totals` - Accumulated summary across all processed files
/// * `file_count` - Total number of files processed
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing to stdout fails.
pub(crate) fn print_list_totals(totals: ListSummary, file_count: usize) -> Result<()> {
    // This matches the separator line printed by upstream `xz -l` for multiple files.
    write_stdout_line(
        "-------------------------------------------------------------------------------",
    )?;

    let ratio = math::ratio_fraction(totals.compressed, totals.uncompressed);
    let check = format_check_name(totals.checks_mask);
    let label = format!("{file_count} files");

    write_stdout_line(&format!(
        "{:>5} {:>7} {:>12} {:>12} {:>6.3}  {:<5}   {}",
        totals.stream_count,
        totals.block_count,
        bytes::format_list_size(totals.compressed),
        bytes::format_list_size(totals.uncompressed),
        ratio,
        check,
        label
    ))?;

    Ok(())
}

/// Write the table header for `xz -l` output if needed.
///
/// The header is printed only when `ctx.print_header` is `true`, which happens
/// for the first file in a multi-file invocation (non-verbose, non-robot mode).
///
/// # Parameters
///
/// * `ctx` - Output context determining whether to print the header
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing to stdout fails.
pub(crate) fn write_list_header_if_needed(ctx: ListOutputContext) -> Result<()> {
    use std::io::Write;

    if !ctx.print_header {
        return Ok(());
    }

    let mut out = io::stdout().lock();
    writeln!(
        out,
        "Strms  Blocks   Compressed Uncompressed  Ratio  Check   Filename"
    )
    .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    Ok(())
}

/// Write a single row for `xz -l` output (non-verbose mode).
///
/// Formats and prints one file's summary information in the standard table format.
///
/// # Parameters
///
/// * `summary` - File summary information to display
/// * `input_path` - Path to the file being listed
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing to stdout fails.
pub(crate) fn write_list_row(summary: ListSummary, input_path: &str) -> Result<()> {
    use std::io::Write;

    let ratio = math::ratio_fraction(summary.compressed, summary.uncompressed);
    let check = format_check_name(summary.checks_mask);

    let mut out = io::stdout().lock();
    writeln!(
        out,
        "{:>5} {:>7} {:>12} {:>12} {:>6.3}  {:<5}   {}",
        summary.stream_count,
        summary.block_count,
        bytes::format_list_size(summary.compressed),
        bytes::format_list_size(summary.uncompressed),
        ratio,
        check,
        input_path
    )
    .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    Ok(())
}

/// Write verbose output for `xz -l -v` mode.
///
/// Prints detailed information about the file, including per-stream and per-block
/// tables. The output format matches upstream `xz -l -v`.
///
/// # Parameters
///
/// * `input_path` - Path to the file being listed
/// * `ctx` - Output context for multi-file formatting (displays `(i/n)` prefix)
/// * `summary` - Overall file summary
/// * `streams` - Per-stream information to display
/// * `blocks` - Per-block information to display (should be sorted by `number_in_file`)
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing to stdout fails.
pub(crate) fn write_verbose_report(
    input_path: &str,
    ctx: ListOutputContext,
    summary: ListSummary,
    streams: &[StreamInfo],
    blocks: &[BlockInfo],
) -> Result<()> {
    use std::io::Write;

    let ratio = math::ratio_fraction(summary.compressed, summary.uncompressed);
    let check = format_check_name(summary.checks_mask);
    let padding_total: u64 = streams.iter().map(|s| s.padding).sum();

    let mut out = io::stdout().lock();
    writeln!(out, "{input_path} ({}/{})", ctx.file_index, ctx.file_count)
        .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    writeln!(out, "  Streams:           {}", summary.stream_count)
        .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    writeln!(out, "  Blocks:            {}", summary.block_count)
        .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    writeln!(
        out,
        "  Compressed size:   {}",
        bytes::format_list_size_with_bytes(summary.compressed)
    )
    .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    writeln!(
        out,
        "  Uncompressed size: {}",
        bytes::format_list_size_with_bytes(summary.uncompressed)
    )
    .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    writeln!(out, "  Ratio:             {ratio:.3}")
        .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    writeln!(out, "  Check:             {check}")
        .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    writeln!(
        out,
        "  Stream Padding:    {}",
        bytes::format_list_size(padding_total)
    )
    .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;

    writeln!(out, "  Streams:").map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    writeln!(
        out,
        "    Stream    Blocks      CompOffset    UncompOffset        CompSize      UncompSize  Ratio  Check      Padding"
    )
    .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;

    for stream in streams {
        let stream_ratio = math::ratio_fraction(stream.compressed_size, stream.uncompressed_size);
        writeln!(
            out,
            "{:>10} {:>9} {:>15} {:>15} {:>15} {:>15}  {:>5.3}  {:<5} {:>12}",
            stream.number,
            stream.block_count,
            stream.compressed_offset,
            stream.uncompressed_offset,
            stream.compressed_size,
            stream.uncompressed_size,
            stream_ratio,
            check,
            stream.padding
        )
        .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    }

    writeln!(out, "  Blocks:").map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    writeln!(
        out,
        "    Stream     Block      CompOffset    UncompOffset       TotalSize      UncompSize  Ratio  Check"
    )
    .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;

    let mut stream_idx: usize = 0;
    let mut remaining_in_stream: u64 = streams.get(stream_idx).map(|s| s.block_count).unwrap_or(0);

    for block in blocks {
        while remaining_in_stream == 0 && stream_idx + 1 < streams.len() {
            stream_idx += 1;
            remaining_in_stream = streams[stream_idx].block_count;
        }
        let stream_number = streams.get(stream_idx).map(|s| s.number).unwrap_or(0);
        remaining_in_stream = remaining_in_stream.saturating_sub(1);

        let block_ratio = math::ratio_fraction(block.total_size, block.uncompressed_size);
        writeln!(
            out,
            "{:>10} {:>9} {:>15} {:>15} {:>15} {:>15}  {:>5.3}  {}",
            stream_number,
            block.number_in_stream,
            block.compressed_file_offset,
            block.uncompressed_file_offset,
            block.total_size,
            block.uncompressed_size,
            block_ratio,
            check
        )
        .map_err(|source| DiagnosticCause::from(Error::WriteOutput { source }))?;
    }

    Ok(())
}
