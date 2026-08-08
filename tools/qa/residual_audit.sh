#!/usr/bin/env sh
# Residual audit — product surface green + intentional negatives documented.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
if [ -f "$repo/compiler/Cargo.toml" ]; then
  comp="$repo/compiler"
else
  comp="$repo"
fi
cd "$comp"

echo "== LANG-RES product surface (must pass) =="
cargo test -p ori-driver --test concurrency_async \
  compile_runs_lang_res_product_surface_native -- --nocapture

echo "== intentional residual: for without iterator ABI =="
cargo test -p ori-driver --test concurrency_async \
  compile_rejects_for_iterable_without_native_abi -- --nocapture 2>/dev/null \
  || cargo test -p ori-driver --test concurrency_async for_iterable -- --nocapture

echo ""
echo "Documented residuals: docs/spec/14-backend-support.md"
echo "Policy: docs/planning/historico/lang-res-closure.md + docs/planning/qa/residual-cleanup-2026-07-13.md"
echo "residual_audit: OK"
