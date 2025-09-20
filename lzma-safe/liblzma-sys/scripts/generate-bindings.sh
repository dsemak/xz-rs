#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="${SCRIPT_DIR}/.."
cd "${CRATE_DIR}"

HEADER="lzma.h"
OUTPUT="src/lzma_bindings.rs"
TMP_FILE="$(mktemp)"
trap 'rm -f "${TMP_FILE}"' EXIT

if ! command -v bindgen >/dev/null 2>&1; then
  echo "error: bindgen CLI not found (install with 'cargo install bindgen-cli')" >&2
  exit 1
fi

INCLUDE_FLAGS=()
if [[ -d "xz/src/liblzma/api" ]]; then
  INCLUDE_FLAGS+=("-Ixz/src/liblzma/api")
fi
if [[ -d "xz/src/common" ]]; then
  INCLUDE_FLAGS+=("-Ixz/src/common")
fi
if [[ -n "${LZMA_INCLUDE_DIR:-}" ]]; then
  INCLUDE_FLAGS+=("-I${LZMA_INCLUDE_DIR}")
fi

if [[ ${#INCLUDE_FLAGS[@]} -eq 0 ]]; then
  echo "warning: no include directories detected; bindgen may fail" >&2
fi

echo "Generating ${OUTPUT}..."
BINDGEN_ARGS=(
  "--allowlist-function" "lzma_.*"
  "--allowlist-type" "lzma_.*"
  "--allowlist-var" "LZMA_.*"
  "--blocklist-type" "max_align_t"
  "--no-layout-tests"
)

bindgen "${HEADER}" "${BINDGEN_ARGS[@]}" -o "${TMP_FILE}" -- "${INCLUDE_FLAGS[@]}"

if command -v rustfmt >/dev/null 2>&1; then
  rustfmt "${TMP_FILE}"
fi

mv "${TMP_FILE}" "${OUTPUT}"
trap - EXIT
echo "Updated ${OUTPUT}."
