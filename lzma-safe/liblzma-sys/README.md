liblzma-sys
===========

Low-level FFI bindings to `liblzma` (XZ Utils) that power the higher level
`lzma-safe` crate. The build script is designed to be reproducible, offline,
and security-conscious while remaining convenient for day-to-day development.

What you get
------------

- **System-first linking:** `pkg-config` is used when available and the probed
  version passes our safety gate.
- **Vendored fallback:** A vendored copy of XZ Utils (`./xz`) is built when
  patches are present, the system copy is missing, or the caller forces it.
- **Pre-generated bindings:** `src/lzma_bindings.rs` ships in-tree for
  environments without `bindgen`; they can be regenerated with a helper script.
- **Security checks:** Known-bad CVEs/commits are blocked by default.
- **Workflow helpers:** Scripts automate binding regeneration, updating
  `xz/`, and smoke-testing the major build permutations.

Feature flags
-------------

- `pkg-config` *(default)* – allow `build.rs` to probe for a system liblzma via
  `pkg-config`.
- `bindgen` *(default in this repo)* – generate bindings at build time instead
  of using the pre-generated `src/lzma_bindings.rs`.

Environment variables
---------------------

- `LZMA_INCLUDE_DIR` – extra include directory passed to bindgen/clang
  invocations.
- `LIBLZMA_STATIC` – set to force the final link step to prefer
  `-l:static=lzma`.
- `LIBLZMA_SYS_FORCE_LOCAL` – disable system detection and always build the
  vendored copy (handy when shipping patched sources).
- `LIBLZMA_SYS_ALLOW_UNSAFE` – override the CVE guard rails. Use only when
  applying out-of-tree fixes to a vulnerable upstream release.

Build flow in a nutshell
------------------------

1. **Detect patches:** Any file in `patches/` makes the build pick the vendored
   lib unconditionally (patches are applied with `patch --forward`).
2. **Choose liblzma implementation:**
   - With `pkg-config` enabled and no patches, probe the system library.
   - Reject versions covering known CVEs.
   - If probing fails (or we are forced local) build `xz/` with the bundled
     `cc` configuration.
3. **Version hardening:** Vendored builds parse `xz/src/liblzma/api/lzma/version.h`
   and ensure the version/commit is not in the deny-list.
4. **Binding generation:** When the `bindgen` feature is active, regenerate
   bindings using the include paths discovered above; otherwise
   `src/lzma_bindings.rs` is used verbatim.

Tracked CVEs
------------

- **CVE-2025-31115** – multithreaded decoder use-after-free in 5.3.3alpha through
  5.8.0; fixed in 5.8.1 / backports.
- **CVE-2024-3094** – supply-chain backdoor in the 5.6.0/5.6.1 release tarballs.
- **CVE-2024-47611** – Windows argument injection in command-line tools (not a
  lib issue but documented for awareness).
- **CVE-2022-1271** – `xzgrep` command injection (scripts live in the vendored
  tree).

Scripts
-------

The `scripts/` directory contains small helpers to keep common workflows tidy:

- `generate-bindings.sh` – regenerates `src/lzma_bindings.rs` from the headers
  currently in use. Respects `LZMA_INCLUDE_DIR` and formats output with
  `rustfmt` when available.
- `update-vendored.sh` – checks out a specific XZ tag/commit in `./xz`, updates
  the build metadata in `Cargo.toml`, and refreshes the bindings. Usage:
  `./scripts/update-vendored.sh [--tag vX.Y.Z | --commit <sha>]`.
- `check-builds.sh` – runs `cargo check` across the four main build modes:
  system default, forced vendored, `bindgen`-only, and pre-generated bindings.

Working with patches
--------------------

Drop `.diff` or `.patch` files into the `patches/` directory. They are applied
in sorted order at build time with `patch --forward` (so re-running the build is
idempotent). Because patches force the vendored build path, the compiled
library always reflects your modifications.

Updating XZ Utils
-----------------

1. Decide which upstream git tag or commit to use.
2. Run `./scripts/update-vendored.sh --tag v5.8.1` (or `--commit <sha>`).
3. Inspect the diff (vendored sources, bindings, `Cargo.toml`).
4. Update the top-level workspace crates if desired, then run
   `./scripts/check-builds.sh` for sanity.

Generating bindings manually
----------------------------

When the `bindgen` feature is disabled you can still refresh the pre-generated
file to match the vendored headers:

```bash
LZMA_INCLUDE_DIR=/opt/liblzma/include \
  ./scripts/generate-bindings.sh
```

The output overwrites `src/lzma_bindings.rs` so remember to review the diff.

Testing build permutations
--------------------------

To exercise all supported configurations:

```bash
./scripts/check-builds.sh
```

This performs:

- `cargo check`
- `LIBLZMA_SYS_FORCE_LOCAL=1 cargo check`
- `cargo check --no-default-features --features bindgen`
- `cargo check --no-default-features`

Licensing
---------

- Crate glue code: MIT OR Apache-2.0.
- Vendored XZ sources: upstream licenses located under `xz/`.
