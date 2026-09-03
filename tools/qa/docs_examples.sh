#!/usr/bin/env sh
# Validate canonical examples referenced by the user documentation.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)

"$repo/tools/qa/docs_coverage.sh"

if [ -n "${ORI_BIN:-}" ]; then
    ori_bin="$ORI_BIN"
elif [ -x "$repo/compiler/target/debug/ori" ]; then
    ori_bin="$repo/compiler/target/debug/ori"
elif [ -x "$repo/compiler/target/release/ori" ]; then
    ori_bin="$repo/compiler/target/release/ori"
else
    ori_bin="ori"
fi

echo "== canonical example checks =="
ORI_BIN="$ori_bin" "$repo/tools/qa/examples_smoke.sh"

if command -v "$ori_bin" >/dev/null 2>&1 || [ -x "$ori_bin" ]; then
    echo "== inline/sidecar documentation check =="
    (
        cd "$repo/compiler"
        "$ori_bin" doc check ..
    )
else
    echo "docs_examples: compiler not found; skipped ori doc check" >&2
fi

echo "docs_examples: canonical examples and documentation references are valid"
