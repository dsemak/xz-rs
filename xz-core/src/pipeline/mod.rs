//! Pipeline module for XZ compression and decompression operations.

#[cfg(feature = "async")]
mod r#async;
mod sync;

#[cfg(feature = "async")]
pub use r#async::{compress_async, decompress_async};
pub use sync::{compress, decompress};

#[cfg(test)]
mod tests {
    use std::io;

    use tokio::io::AsyncRead;

    /// Sample text data for testing compression/decompression functionality
    pub const SAMPLE: &[u8] = b"The quick brown fox jumps over the lazy dog";

    /// Large sample data (1MB) for testing performance and memory handling
    pub const LARGE_SAMPLE: &[u8] = &[b'A'; 1024 * 1024];

    /// Empty sample for testing edge cases with zero-length input
    pub const EMPTY_SAMPLE: &[u8] = b"";

    /// A reader that simulates slow I/O by reading data in small chunks.
    ///
    /// This is useful for testing streaming behavior and ensuring that
    /// compression/decompression works correctly with partial reads.
    pub struct SlowReader<'a> {
        data: &'a [u8],
        pos: usize,
        chunk_size: usize,
    }

    impl<'a> SlowReader<'a> {
        /// Creates a new slow reader that will read at most `chunk_size` bytes per operation.
        pub fn new(data: &'a [u8], chunk_size: usize) -> Self {
            Self {
                data,
                pos: 0,
                chunk_size,
            }
        }
    }

    #[cfg(feature = "async")]
    impl AsyncRead for SlowReader<'_> {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            let remaining = self.data.len() - self.pos;
            if remaining == 0 {
                return std::task::Poll::Ready(Ok(()));
            }

            // Read at most chunk_size bytes, limited by remaining data and buffer capacity
            let to_read = std::cmp::min(self.chunk_size, std::cmp::min(remaining, buf.capacity()));
            let end = self.pos + to_read;
            buf.put_slice(&self.data[self.pos..end]);
            self.pos = end;

            std::task::Poll::Ready(Ok(()))
        }
    }

    impl io::Read for SlowReader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let remaining = self.data.len() - self.pos;
            if remaining == 0 {
                return Ok(0);
            }

            // Read at most chunk_size bytes, limited by remaining data and buffer size
            let to_read = std::cmp::min(self.chunk_size, std::cmp::min(remaining, buf.len()));
            let end = self.pos + to_read;
            buf[..to_read].copy_from_slice(&self.data[self.pos..end]);
            self.pos = end;

            Ok(to_read)
        }
    }

    /// A reader that simulates I/O failures after reading a specified number of bytes.
    ///
    /// This is useful for testing error handling and recovery mechanisms
    /// in compression/decompression pipelines.
    pub struct FailingReader {
        fail_after: usize,
        bytes_read: usize,
    }

    impl FailingReader {
        /// Creates a new failing reader that will fail after reading `fail_after` bytes.
        pub fn new(fail_after: usize) -> Self {
            Self {
                fail_after,
                bytes_read: 0,
            }
        }
    }

    impl io::Read for FailingReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.bytes_read >= self.fail_after {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Simulated I/O error",
                ));
            }

            // Read one byte at a time to provide predictable failure point
            if buf.is_empty() {
                return Ok(0);
            }

            buf[0] = b'A';
            self.bytes_read += 1;
            Ok(1)
        }
    }

    #[cfg(feature = "async")]
    impl AsyncRead for FailingReader {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            if self.bytes_read >= self.fail_after {
                return std::task::Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Simulated I/O error",
                )));
            }

            // Read one byte at a time to provide predictable failure point
            if buf.capacity() > 0 {
                buf.put_slice(b"A");
                self.bytes_read += 1;
            }

            std::task::Poll::Ready(Ok(()))
        }
    }

    /// A writer that simulates I/O failures after writing a specified number of bytes.
    ///
    /// This is useful for testing error handling during the output phase
    /// of compression/decompression operations.
    pub struct FailingWriter {
        fail_after: usize,
        bytes_written: usize,
    }

    impl FailingWriter {
        /// Creates a new failing writer that will fail after writing `fail_after` bytes.
        pub fn new(fail_after: usize) -> Self {
            Self {
                fail_after,
                bytes_written: 0,
            }
        }
    }

    impl io::Write for FailingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.bytes_written >= self.fail_after {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "Simulated write error",
                ));
            }

            // Write as much as possible before hitting the failure threshold
            let to_write = std::cmp::min(buf.len(), self.fail_after - self.bytes_written);
            self.bytes_written += to_write;
            Ok(to_write)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
