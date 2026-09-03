#!/usr/bin/env sh
# LANG-PERF-2-0: quick polyglot smoke (fib + list) using system ori or PATH.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
poly="$root/tools/bench/polyglot"
ORI_BIN="${ORI_BIN:-$(command -v ori)}"
export ORI_USE_SYSTEM_LINKER="${ORI_USE_SYSTEM_LINKER:-}"
# Prefer clean linker defaults
unset ORI_USE_SYSTEM_LINKER

echo "== perf_polyglot_smoke: ori=$ORI_BIN =="
"$ORI_BIN" --version

mkdir -p "$poly/bin"
for w in fib_iter list_sum; do
  echo "  compile $w"
  tmp="/tmp/ori_smoke_${w}_$$"
  "$ORI_BIN" compile "$poly/ori/${w}.orl" --out "$tmp"
  mv -f "$tmp" "$poly/bin/ori_${w}"
  out=$("$poly/bin/ori_${w}")
  echo "  run $w -> $out"
done
echo "OK perf_polyglot_smoke"
