#!/usr/bin/env sh
# Validate the documentation example corpus with the real Ori compiler.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)

"$repo/tools/qa/docs_coverage.sh"
"$repo/tools/qa/examples_smoke.sh"

if [ -n "${ORI_BIN:-}" ]; then
    ori_bin=$ORI_BIN
elif [ -x "$repo/compiler/target/debug/ori" ]; then
    ori_bin="$repo/compiler/target/debug/ori"
elif [ -x "$repo/compiler/target/release/ori" ]; then
    ori_bin="$repo/compiler/target/release/ori"
else
    ori_bin=ori
fi

if command -v "$ori_bin" >/dev/null 2>&1 || [ -x "$ori_bin" ]; then
    (
        cd "$repo/compiler"
        "$ori_bin" doc check ..
    )
else
    echo "docs_examples: compiler not found; skipped ori doc check" >&2
fi

echo "docs_examples: documentation examples and inline docs are valid"
