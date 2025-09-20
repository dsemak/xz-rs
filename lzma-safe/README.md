# lzma-safe

Safe, high-level bindings to the [`liblzma`](https://tukaani.org/xz/) encoder and decoder used by XZ Utils.
The crate wraps the raw C API in resource-owning Rust types, providing idiomatic streaming compression and
decompression without sprinkling `unsafe` throughout your codebase.

## Features

- RAII management of `lzma_stream` and allocator state
- Streaming encoder/decoder with detailed error mapping
- Presets, multi-threaded mode, and custom filter chains
- Optional custom memory allocators via a small trait
- Works with XZ containers as well as legacy `.lzma` streams

## Installation

```toml
[dependencies]
lzma-safe = "0.1"
```

The bundled `liblzma-sys` crate expects the system to provide liblzma. Install the development headers for
your platform (for example `sudo apt install liblzma-dev` on Debian-based systems). See `liblzma-sys` for details.

## Quick start

```rust
use lzma_safe::{Action, Stream};
use lzma_safe::encoder::options::{Compression, IntegrityCheck};

fn roundtrip(data: &[u8]) -> Result<Vec<u8>, lzma_safe::Error> {
    // Compress
    let stream = Stream::default();
    let mut encoder = stream.easy_encoder(Compression::Level6, IntegrityCheck::Crc64)?;
    let mut buffer = vec![0_u8; 256];
    let (_, len) = encoder.process(data, &mut buffer, Action::Finish)?;
    buffer.truncate(len);

    // Decompress
    let stream = Stream::default();
    let mut decoder = stream.auto_decoder(u64::MAX, Default::default())?;
    let mut out = vec![0_u8; data.len()];
    let (_, len) = decoder.process(&buffer, &mut out, Action::Finish)?;
    out.truncate(len);
    Ok(out)
}
```

## Customisation

- `Stream::multithreaded_encoder`/`Stream::mt_decoder` enable multi-threaded compression when the linked
  liblzma supports it.
- Configure presets, filter chains, and integrity checks through `encoder::options`.
- Fine-tune decoder behaviour and memory limits via `decoder::options`.
- Implement `stream::Allocator` and pass it to `Stream::with_allocator` to track or customise allocations.

## License

`lzma-safe` is distributed under the terms of the MIT license. The crate depends on the system `liblzma`
library, which is available under the LGPL v2.1-or-later.
