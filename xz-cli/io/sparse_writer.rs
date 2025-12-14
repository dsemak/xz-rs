//! Sparse output writer implementation.

use std::fs::File;
use std::io;
use std::io::{Seek, SeekFrom, Write};

use crate::config::DEFAULT_BUFFER_SIZE;

/// A writer that attempts to create sparse files by turning long runs of zeros into holes.
///
/// This is used to mimic upstream `xz` behavior when decompressing to a regular file.
pub(crate) struct SparseFileWriter {
    inner: io::BufWriter<File>,
    pending_zeros: usize,
    logical_pos: u64,
    threshold: usize,
}

impl SparseFileWriter {
    /// Creates a sparse writer using a conservative default threshold.
    pub(crate) fn new(file: File) -> Self {
        Self::with_threshold(file, Self::default_threshold())
    }

    /// Creates a sparse writer that will turn zero runs >= `threshold` into holes.
    pub(crate) fn with_threshold(file: File, threshold: usize) -> Self {
        Self {
            inner: io::BufWriter::with_capacity(DEFAULT_BUFFER_SIZE, file),
            pending_zeros: 0,
            logical_pos: 0,
            threshold: threshold.max(1),
        }
    }

    const fn default_threshold() -> usize {
        // Upstream `xz` uses sparse output by default when possible. We keep the threshold
        // conservative to avoid excessive seeks for short zero runs.
        4096
    }

    fn write_literal_zeros(&mut self, mut len: usize) -> io::Result<()> {
        const ZERO_BUF: [u8; 8192] = [0; 8192];

        while len > 0 {
            let chunk = len.min(ZERO_BUF.len());
            self.inner.write_all(&ZERO_BUF[..chunk])?;
            self.logical_pos = self
                .logical_pos
                .checked_add(chunk as u64)
                .ok_or_else(|| io::Error::other("Sparse write overflow"))?;
            len -= chunk;
        }
        Ok(())
    }

    fn flush_pending_zeros(&mut self) -> io::Result<()> {
        if self.pending_zeros == 0 {
            return Ok(());
        }

        let pending = self.pending_zeros;
        self.pending_zeros = 0;

        if pending >= self.threshold {
            // Flush buffered data before seeking.
            self.inner.flush()?;

            let new_pos = self
                .logical_pos
                .checked_add(pending as u64)
                .ok_or_else(|| io::Error::other("Sparse seek overflow"))?;

            self.inner.get_mut().seek(SeekFrom::Start(new_pos))?;
            self.logical_pos = new_pos;
            Ok(())
        } else {
            self.write_literal_zeros(pending)
        }
    }

    fn finalize_len(&mut self) -> io::Result<()> {
        // If we have an unmaterialized hole at EOF, we must still ensure the file length matches
        // the logical length.
        self.inner.flush()?;
        self.inner.get_mut().set_len(self.logical_pos)?;
        Ok(())
    }
}

impl io::Write for SparseFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // We always consume the whole buffer or return an error.
        self.write_all(buf)?;
        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let mut i = 0usize;
        while i < buf.len() {
            // Collect zero run.
            if buf[i] == 0 {
                let start = i;
                while i < buf.len() && buf[i] == 0 {
                    i += 1;
                }
                self.pending_zeros = self.pending_zeros.saturating_add(i - start);
                if self.pending_zeros >= self.threshold {
                    self.flush_pending_zeros()?;
                }
                continue;
            }

            // Before writing non-zero bytes, materialize pending zeros.
            self.flush_pending_zeros()?;

            let start = i;
            while i < buf.len() && buf[i] != 0 {
                i += 1;
            }
            self.inner.write_all(&buf[start..i])?;
            self.logical_pos = self
                .logical_pos
                .checked_add((i - start) as u64)
                .ok_or_else(|| io::Error::other("Sparse write overflow"))?;
        }

        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_pending_zeros()?;
        self.finalize_len()?;
        self.inner.flush()
    }
}

impl Drop for SparseFileWriter {
    fn drop(&mut self) {
        // Best-effort finalize; ignore errors on drop.
        let _ = self.flush();
    }
}
