#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: update-vendored.sh [--tag <tag>] [--commit <sha>] [--no-fetch]

Without options the script fetches upstream tags and checks out the latest
stable release (vX.Y.Z) of XZ Utils in the vendored `xz/` directory. After the
checkout it regenerates pre-generated bindings and refreshes the crate version
metadata to reflect the new upstream release.
USAGE
}

TAG=""
COMMIT=""
DO_FETCH=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)
      TAG="$2"
      shift 2
      ;;
    --commit)
      COMMIT="$2"
      shift 2
      ;;
    --no-fetch)
      DO_FETCH=0
      shift 1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -n "$TAG" && -n "$COMMIT" ]]; then
  echo "error: specify either --tag or --commit, not both" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="${SCRIPT_DIR}/.."
cd "${CRATE_DIR}"

if [[ ! -d xz/.git ]]; then
  echo "error: xz/ is missing or not a git clone" >&2
  exit 1
fi

if (( DO_FETCH )); then
  git -C xz fetch --tags --prune
fi

TARGET=""
if [[ -n "$COMMIT" ]]; then
  TARGET="$COMMIT"
else
  if [[ -z "$TAG" ]]; then
    TAG="$(git -C xz tag -l 'v[0-9]*.[0-9]*.[0-9]*' | sort -V | tail -n1)"
    if [[ -z "$TAG" ]]; then
      echo "error: unable to determine latest release tag" >&2
      exit 1
    fi
  fi
  TARGET="$TAG"
fi

echo "Checking out ${TARGET} in xz/"
git -C xz checkout "$TARGET"

# Capture resolved tag/commit info for logging.
RESOLVED_TAG=$(git -C xz describe --tags --exact-match 2>/dev/null || true)
RESOLVED_COMMIT=$(git -C xz rev-parse --short HEAD)

# Derive liblzma version components from version.h.
VERSION_HEADER="xz/src/liblzma/api/lzma/version.h"
if [[ ! -f "$VERSION_HEADER" ]]; then
  echo "error: ${VERSION_HEADER} not found after checkout" >&2
  exit 1
fi

extract_define() {
  local name="$1"
  awk -v n="$name" '$2 == n { print $3 }' "$VERSION_HEADER"
}

MAJOR=$(extract_define LZMA_VERSION_MAJOR)
MINOR=$(extract_define LZMA_VERSION_MINOR)
PATCH=$(extract_define LZMA_VERSION_PATCH)
SUFFIX=$(awk '$2 == "LZMA_VERSION_COMMIT" { gsub(/"/,"",$3); print $3 }' "$VERSION_HEADER")

if [[ -z "$MAJOR" || -z "$MINOR" || -z "$PATCH" ]]; then
  echo "error: failed to parse liblzma version from ${VERSION_HEADER}" >&2
  exit 1
fi

XZ_VERSION="${MAJOR}.${MINOR}.${PATCH}${SUFFIX}"

# Update crate version metadata to match the vendored release.
update_version_field() {
  local file="$1"
  local new_value="$2"
  python3 - "$file" "$new_value" <<'PY'
import pathlib, re, sys
path = pathlib.Path(sys.argv[1])
value = sys.argv[2]
text = path.read_text()
pattern = re.compile(r'^(version\s*=\s*")([^"\n]+)(")', re.MULTILINE)
match = pattern.search(text)
if not match:
    sys.exit("failed to locate version field in {}".format(path))
base = match.group(2).split('+', 1)[0]
replacement = f"{base}+xz.{value}"
new_text = text[:match.start(2)] + replacement + text[match.end(2):]
path.write_text(new_text)
PY
}

update_version_field "Cargo.toml" "$XZ_VERSION"

echo "Vendored liblzma now at ${XZ_VERSION} (${RESOLVED_TAG:-commit ${RESOLVED_COMMIT}})"

echo "Regenerating pre-generated bindings"
"${SCRIPT_DIR}/generate-bindings.sh"

echo "Done. Review changes in git status and update dependant crates as needed."
