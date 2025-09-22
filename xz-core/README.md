# xz-core

`xz-core` provides a high-level, memory-safe streaming pipeline for
XZ (LZMA2) compression and decompression built on top of the `lzma-safe` bindings to liblzma.
It offers synchronous and asynchronous adapters, strong defaults for security-sensitive workloads,
and ergonomic configuration builders for production deployments.

## Features

- Streaming compressors/decompressors that work with any `Read`/`Write` or async `AsyncRead`/`AsyncWrite` sources
- Configurable threading, memory budgets, block sizes, filter chains, timeouts, and integrity checks
- Security-focused defaults that mitigate decompression bombs and allow strict memory ceilings
- Optional Tokio-powered async pipeline (`async` feature enabled by default)
- Custom buffer management APIs (`Buffer`, `Allocator`, `Deallocator`) for integrating with bespoke memory strategies
- Friendly error model that surfaces I/O errors, backend issues, and misconfiguration via a single `Error` enum

## Getting Started

Add the crate to your `Cargo.toml`. The crate is part of the [`xz-rs`](https://github.com/dsemak/xz-rs) workspace
and can be consumed either by version (once published) or via Git:

```toml
[dependencies]
xz-core = "0.1.1"
```

### Feature Flags

- `async` *(default)* – enables Tokio-based async helpers (`compress_async`, `decompress_async`).
Disable it with `default-features = false` if you only need the blocking API.

## Synchronous Pipeline

```rust
use std::io::Cursor;

use xz_core::{
    options::{CompressionOptions, DecompressionOptions},
    pipeline,
    Result,
};

fn main() -> Result<()> {
    let input = b"The quick brown fox jumps over the lazy dog";

    // Compress into an in-memory buffer
    let mut compressed = Vec::new();
    let compress_summary = pipeline::compress(
        Cursor::new(&input[..]),
        &mut compressed,
        &CompressionOptions::default(),
    )?;
    println!(
        "compressed {} bytes into {} bytes",
        compress_summary.bytes_read,
        compress_summary.bytes_written
    );

    // Decompress back into plain text
    let mut output = Vec::new();
    let decompress_summary = pipeline::decompress(
        Cursor::new(compressed.as_slice()),
        &mut output,
        &DecompressionOptions::default(),
    )?;
    println!(
        "decompressed {} bytes into {} bytes",
        decompress_summary.bytes_read,
        decompress_summary.bytes_written
    );

    assert_eq!(output, input);
    Ok(())
}
```

## Async Pipeline

```rust
use std::io::Cursor;

use tokio::io::duplex;
use tokio::task;

use xz_core::{
    options::{CompressionOptions, DecompressionOptions},
    pipeline,
    Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    let input = b"The quick brown fox jumps over the lazy dog";
    let mut compressed = Vec::new();

    // Compress from a reader into an in-memory buffer
    pipeline::compress_async(
        Cursor::new(&input[..]),
        &mut compressed,
        &CompressionOptions::default(),
    )
    .await?;

    // Stream decompression using an async pipe
    let (mut writer, mut reader) = duplex(64 * 1024);
    task::spawn(async move {
        let _ = writer.write_all(&compressed).await;
        writer.shutdown().await.unwrap();
    });

    let mut output = Vec::new();
    pipeline::decompress_async(
        &mut reader,
        &mut output,
        &DecompressionOptions::default(),
    )
    .await?;

    assert_eq!(output, input);
    Ok(())
}
```

## Configuring Compression & Decompression

```rust
use std::num::NonZeroU64;
use std::time::Duration;

use lzma_safe::encoder::options::{Compression, IntegrityCheck};

use xz_core::{
    config::DecodeMode,
    options::{CompressionOptions, DecompressionOptions},
    Threading,
};

let compression = CompressionOptions::default()
    .with_level(Compression::Level9)
    .with_check(IntegrityCheck::Sha256)
    .with_threads(Threading::Exact(8))
    .with_block_size(NonZeroU64::new(16 * 1024 * 1024))
    .with_timeout(Some(Duration::from_secs(10)));

let decompression = DecompressionOptions::default()
    .with_threads(Threading::Auto)
    .with_memlimit(NonZeroU64::new(64 * 1024 * 1024).unwrap())
    .with_memlimit_stop(Some(NonZeroU64::new(128 * 1024 * 1024).unwrap()))
    .with_mode(DecodeMode::Xz)
    .with_timeout(Some(Duration::from_secs(5)));
```

Key knobs:

- `CompressionOptions` controls presets, filter chains, threading, timeouts, and buffer sizes.
- `DecompressionOptions` enforces memory ceilings, decoder flags, format auto-detection, and threading rules.
- `Threading` intelligently caps worker counts to avoid starving the host,
while `DecodeMode` lets you pick between XZ, legacy LZMA, or auto-detection.

## Memory & Buffer Management

The pipeline allocates scratch buffers via `Buffer`.
You can plug in a custom allocator by implementing `Allocator` and handing it to `Buffer::with_allocator`.
Built-in helpers zero buffers on allocation and wipe them on drop to reduce information leakage.

## Error Handling

All fallible APIs return `xz_core::Result<T>` with the crate-wide `Error` enum. It distinguishes I/O failures,
liblzma backend errors, invalid configuration, unsafe thread counts, allocation limits,
and compromised backend detection, while preserving the original sources for debugging.

## Testing

Run the crate's unit and integration suite with:

```bash
cargo test -p xz-core
```

## License

Licensed under the MIT License, the same as the rest of the `xz-rs` workspace. See [`LICENSE`](../LICENSE).
