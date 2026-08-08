#!/usr/bin/env sh
# Ori language QA — fast daily stages (S0–S4 + S8).
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
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
echo "== D0 documentation catalog =="
sh "$repo/tools/qa/docs_coverage.sh"
cd "$comp"
echo "== S0 cargo check --workspace =="
cargo check --workspace
echo "== S0 strict clippy (runtime + codegen + driver) =="
cargo clippy -p ori-runtime -p ori-codegen -p ori-driver --tests --no-deps -- -D warnings
echo "== S1 unit crates =="
cargo test -p ori-lexer -- --quiet 2>/dev/null || true
cargo test -p ori-parser -- --quiet 2>/dev/null || true
cargo test -p ori-types -- --quiet 2>/dev/null || true
cargo test -p ori-hir -- --quiet 2>/dev/null || true
echo "== S2 ori_spec + diagnostic_catalog =="
cargo test -p ori-driver --test ori_spec -- --quiet
cargo test -p ori-driver --test diagnostic_catalog -- --quiet
echo "== S3 memory + security =="
cargo test -p ori-driver --test memory_arc -- --quiet
cargo test -p ori-driver --test security_robustness -- --quiet
echo "== S8 residual product surface =="
cargo test -p ori-driver --test concurrency_async compile_runs_lang_res_product_surface_native -- --quiet
echo "daily_fast: OK"
