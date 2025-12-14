Rust Compression Utilities

Modern, safe, and efficient compression utilities written in pure Rust. This project provides two independent implementations:

· gzip-utils: Fast gzip compression/decompression using flate2
· xz-utils: XZ/LZMA compression/decompression using pure-Rust lzma-rs

🚀 Features

✅ gzip-utils (gzip, gunzip, zcat)

· 100% Safe Rust - No unsafe code, no C dependencies
· Fast performance - Optimized for modern hardware
· Full gzip compatibility - Works with standard .gz files
· Streaming support - stdin/stdout pipelines
· Multi-file processing - Process multiple files at once

✅ xz-utils (xz, unxz, xzcat)

· Pure Rust LZMA - No vulnerable C libraries (immune to CVE-2024-3094)
· Dual format support - .xz (LZMA2) and .lzma formats
· Auto-detection - Automatically detects compression format
· Safe by design - Memory safe, thread safe
· Cross-platform - Works anywhere Rust runs

📦 Installation

From Source

```bash
git clone https://github.com/yourusername/xz-rs.git
cd xz-rs
cargo build --release
```

Install Binaries

```bash
# Install all utilities
cargo install --path gzip-utils
cargo install --path xz-utils

# Or install individually
cargo install --path gzip-utils --bin gzip
cargo install --path xz-utils --bin xz
```

🛠️ Usage

gzip-utils

```bash
# Compress files
gzip file.txt                 # Creates file.txt.gz
gzip -9 file.txt             # Maximum compression
gzip -k file.txt             # Keep original file

# Decompress files
gunzip file.txt.gz           # Extracts to file.txt
gunzip -f archive.gz         # Force overwrite

# View compressed files
zcat file.txt.gz             # Output to stdout
cat file.txt.gz | zcat       # From stdin

# Multiple files
gzip file1.txt file2.txt file3.txt
```

xz-utils

```bash
# Compress to .xz format (default)
xz file.txt                  # Creates file.txt.xz
xz --format=lzma file.txt    # Legacy .lzma format

# Decompress
unxz file.txt.xz             # Auto-detects format
unxz file.txt.lzma

# View compressed files
xzcat archive.xz             # Like cat for .xz files

# Test integrity
xz -t archive.xz             # Test without extracting
```

Common Options

```bash
-v, --verbose    # Verbose output
-f, --force      # Overwrite existing files
-k, --keep       # Keep original files
-c, --stdout     # Write to stdout
-1 .. -9         # Compression level (1=fast, 9=best)
```

🔧 Advanced Examples

Pipes and Streams

```bash
# Compress streaming output
tar -cf - directory/ | gzip > archive.tar.gz
tar -cf - directory/ | xz > archive.tar.xz

# Process in pipelines
cat logfile.txt | gzip -c | ssh server "gunzip -c > logfile.txt"

# Parallel processing with xargs
find . -name "*.log" -print0 | xargs -0 -P4 gzip
```

Batch Operations

```bash
# Compress all text files
find . -name "*.txt" -exec gzip {} \;

# Decompress all archives
for f in *.gz; do gunzip "$f"; done
for f in *.xz; do unxz "$f"; done
```

🏗️ Architecture

```
xz-rs/
├── gzip-utils/          # Gzip implementation
│   ├── src/            # Library code
│   └── bin/            # gzip, gunzip, zcat binaries
├── xz-utils/           # XZ/LZMA implementation  
│   ├── src/            # Library code
│   └── bin/            # xz, unxz, xzcat binaries
└── Cargo.toml          # Workspace configuration
```

Design Principles

1. Safety First - No unsafe code, no external C dependencies
2. Modularity - Independent utilities with clean separation
3. Performance - Efficient memory usage and streaming
4. Compatibility - Standard command-line interface

🧪 Testing

```bash
# Run all tests
cargo test --workspace

# Test specific utility
cargo test -p gzip-utils
cargo test -p xz-utils

# Integration tests
./test_integration.sh  # See examples below
```

Test Examples

```bash
# Round-trip compression test
echo "Test data" | gzip | gunzip

# Format detection test
xz --format=lzma test.txt
unxz test.txt.lzma

# Large file handling
dd if=/dev/urandom of=test.bin bs=1M count=100
gzip test.bin
xz test.bin
```

📊 Performance

Comparison

Operation gzip-utils xz-utils GNU gzip xz (C)
Compression speed ⚡⚡⚡⚡ ⚡⚡ ⚡⚡⚡⚡⚡ ⚡
Decompression speed ⚡⚡⚡⚡⚡ ⚡⚡⚡ ⚡⚡⚡⚡⚡ ⚡⚡
Compression ratio ⚡⚡⚡ ⚡⚡⚡⚡⚡ ⚡⚡⚡ ⚡⚡⚡⚡⚡
Memory usage ⚡⚡⚡⚡⚡ ⚡⚡⚡⚡ ⚡⚡⚡⚡⚡ ⚡

Note: gzip-utils is optimized for speed, xz-utils for compression ratio.

🔒 Security

Why This Project is Safer

· ✅ No C dependencies - Immune to CVE-2024-3094 (XZ backdoor)
· ✅ Memory safe - Rust guarantees no buffer overflows
· ✅ Sandbox ready - Suitable for containerized environments
· ✅ Auditable - Pure Rust code is easier to review

Security Features

· Safe handling of malformed archives
· Proper resource cleanup (RAII)
· No arbitrary code execution vectors
· Input validation and sanitization

📈 Benchmarks

```bash
# Run included benchmarks
cargo bench -p gzip-utils
cargo bench -p xz-utils

# Quick performance test
time gzip largefile.bin
time xz largefile.bin
```

Development Setup

```bash
git clone https://github.com/yourusername/xz-rs.git
cd xz-rs
cargo build
cargo test
```

Code Standards

· Follow Rustfmt formatting
· Use Clippy for linting
· Write tests for new features
· Document public APIs

📄 License

Licensed under either:

· MIT License (LICENSE-MIT)
· Apache License 2.0 (LICENSE-APACHE)

at your option.

🙏 Acknowledgments

· flate2 - Rust gzip implementation
· lzma-rs - Pure Rust LZMA
· The Rust community for excellent tooling

---

Star this repo if you find it useful! ⭐

---

🚨 Important Security Note

This project was created in response to CVE-2024-3094 (XZ backdoor). By using pure Rust implementations, we eliminate the risk of supply chain attacks through C libraries. Always verify checksums of downloaded binaries.

