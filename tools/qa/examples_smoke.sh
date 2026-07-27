#!/usr/bin/env sh
# Compile (and optionally run) official examples — product surface.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
if [ -z "${ORI_BIN:-}" ]; then
  if [ -x "$repo/compiler/target/debug/ori" ]; then
    ORI_BIN="$repo/compiler/target/debug/ori"
  elif [ -x "$repo/compiler/target/release/ori" ]; then
    ORI_BIN="$repo/compiler/target/release/ori"
  else
    ORI_BIN="ori"
  fi
fi
export ORI_USE_SYSTEM_LINKER="${ORI_USE_SYSTEM_LINKER:-1}"

ex_dir="$repo/examples"
if [ ! -d "$ex_dir" ]; then
  echo "no examples/ at $repo" >&2
  exit 0
fi

# Prefer .orl entrypoints / project dirs
fail=0
ok=0
for f in "$ex_dir"/*/; do
  [ -d "$f" ] || continue
  name=$(basename "$f")
  # skip if no .orl
  if ! ls "$f"*.orl >/dev/null 2>&1 && [ ! -f "$f/ori.pkg.toml" ]; then
    continue
  fi
  entry=""
  if [ -f "$f/main.orl" ]; then
    entry="$f/main.orl"
  else
    entry=$(ls "$f"*.orl 2>/dev/null | head -1 || true)
  fi
  [ -n "$entry" ] || continue
  echo "-- check $name --"
  if "$ORI_BIN" check "$entry" >/dev/null 2>&1; then
    ok=$((ok + 1))
  else
    echo "FAIL check $entry" >&2
    "$ORI_BIN" check "$entry" 2>&1 | head -8 || true
    fail=$((fail + 1))
  fi
done

echo "examples_smoke: $ok ok / $fail fail"
[ "$fail" -eq 0 ]
