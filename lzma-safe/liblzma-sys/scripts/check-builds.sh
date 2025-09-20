#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="${SCRIPT_DIR}/.."
cd "${CRATE_DIR}"

run_case() {
  local name="$1"
  shift
  echo "==> ${name}"
  ( set -x; "$@" )
  echo
}

run_case "default features (system preferred)" \
  cargo check

run_case "forced vendored build" \
  env LIBLZMA_SYS_FORCE_LOCAL=1 cargo check

run_case "bindgen without pkg-config" \
  cargo check --no-default-features --features bindgen

run_case "pre-generated bindings (no bindgen)" \
  cargo check --no-default-features

echo "All build permutations completed."
