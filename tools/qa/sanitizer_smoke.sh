#!/usr/bin/env sh
# Ori language QA - sanitizer smoke for the native runtime (QA-SAN-CI-1).
#
# Runs Rust test suites in ori-runtime and ori-embed under AddressSanitizer
# and ThreadSanitizer using -Zsanitizer on nightly or the stable `-Z` flag
# where supported. Falls back gracefully when the toolchain lacks support:
# reports INCOMPLETE instead of a false success or a spurious failure.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
if [ -f "$repo/compiler/Cargo.toml" ]; then
  comp="$repo/compiler"
elif [ -f "$repo/Cargo.toml" ]; then
  comp="$repo"
else
  echo "cannot find Ori workspace from $repo" >&2
  exit 2
fi
cd "$comp"
if ! rustc -Z help >/dev/null 2>&1; then
  echo "sanitizer_smoke: INCOMPLETE (toolchain lacks -Zsanitizer support; install nightly for sanitizer coverage)"
  exit 0
fi
echo "== sanitizer: ori-runtime unit tests under ASan =="
RUSTFLAGS="-Zsanitizer=address" cargo test -p ori-runtime --lib -- --quiet || {
  echo "sanitizer_smoke: FAILED (ASan ori-runtime)" >&2
  exit 1
}
echo "== sanitizer: ori-runtime unit tests under TSan =="
RUSTFLAGS="-Zsanitizer=thread" cargo test -p ori-runtime --lib -- --quiet || {
  echo "sanitizer_smoke: FAILED (TSan ori-runtime)" >&2
  exit 1
}
echo "== sanitizer: ori-embed tests (ASan) =="
RUSTFLAGS="-Zsanitizer=address" cargo test -p ori-embed -- --quiet || {
  echo "sanitizer_smoke: FAILED (ASan ori-embed)" >&2
  exit 1
}
echo "sanitizer_smoke: OK"
