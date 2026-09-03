#!/usr/bin/env sh
set -eu

# Runner for VALUE-PERF-1 benchmark suite.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
ORI_BIN="${REPO_ROOT}/compiler/target/debug/ori"

if [ ! -f "$ORI_BIN" ]; then
    echo "Building ori compiler..."
    (cd "${REPO_ROOT}/compiler" && cargo build -p ori-driver)
fi

echo "=== Running VALUE-PERF-1 Benchmarks ==="

for bench in vec3_add_loop mat3_multiply optional_scalar_loop; do
    bench_file="${SCRIPT_DIR}/${bench}.orl"
    echo ""
    echo ">> Benchmark: ${bench}"
    time "$ORI_BIN" run "$bench_file"
done

echo ""
echo "=== All benchmarks completed successfully ==="
