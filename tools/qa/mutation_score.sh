#!/usr/bin/env sh
# Informational mutation-testing harness (never a gate).
#
# Measures assertion strength in ori-types/ori-hir via cargo-mutants.
# Requires nightly + cargo-mutants installed. Always informational:
# reports SURVIVED/TIMEOUT counts, never fails the build.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
comp="$repo/compiler"

if ! command -v cargo-mutants >/dev/null 2>&1; then
    echo "mutation_score: INCOMPLETE (cargo-mutants not installed; cargo install cargo-mutants)"
    exit 0
fi

cd "$comp"
cargo mutants --no-shuffle --test-tool cargo -- --manifest-path "$comp/Cargo.toml" \
    -p ori-types -p ori-hir 2>&1 | tail -n 20 || true
echo "mutation_score: informational only (see output above)"
