#!/usr/bin/env sh
# SEC8 golden suite for packages/ori-web (no network).
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)

if [ -x "$repo/compiler/target/debug/ori" ]; then
  ORI_BIN="${ORI_BIN:-$repo/compiler/target/debug/ori}"
elif [ -x "$repo/compiler/target/release/ori" ]; then
  ORI_BIN="${ORI_BIN:-$repo/compiler/target/release/ori}"
else
  ORI_BIN="${ORI_BIN:-ori}"
fi

smoke_root="$repo/packages/ori-web/examples/sec8_tests"
if [ ! -d "$smoke_root" ]; then
  echo "web_sec8: SKIP (package fixture is not present)" >&2
  exit 0
fi
package_cache=$(mktemp -d "${TMPDIR:-/tmp}/ori-web-sec8-cache.XXXXXX")
trap 'rm -rf -- "$package_cache"' EXIT HUP INT TERM
export ORI_PACKAGE_CACHE="$package_cache"
cd "$smoke_root"

echo "== web_sec8: $ORI_BIN run main.orl =="
out=$("$ORI_BIN" run main.orl 2>&1) || {
  echo "$out" >&2
  echo "web_sec8: FAIL (run error)" >&2
  exit 1
}
echo "$out"
echo "$out" | grep -q "SEC8 ALL PASSED" || {
  echo "web_sec8: FAIL (missing SEC8 ALL PASSED)" >&2
  exit 1
}
echo "web_sec8: OK"
