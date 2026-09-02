#!/usr/bin/env sh
# Validate that the runtime exports every symbol consumed by the manifest and
# native backend. This is intentionally small and dependency-light so it runs
# in the required Linux/macOS QA path as well as local development.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
comp="$repo/compiler"
target_dir="${CARGO_TARGET_DIR:-$comp/target}"
profile="${ORI_ABI_PROFILE:-debug}"

cargo_args=""
if [ "$profile" = "release" ]; then
    cargo_args="--release"
elif [ "$profile" != "debug" ]; then
    echo "abi_exports: ORI_ABI_PROFILE must be debug or release" >&2
    exit 2
fi

cargo build --manifest-path "$comp/Cargo.toml" -p ori-runtime --lib --locked $cargo_args >/dev/null

host_target=$(rustc -vV | sed -n 's/^host: //p')
metadata="$repo/runtime/$host_target/runtime-link.json"
if [ -f "$metadata" ]; then
    python3 "$script_dir/validate_runtime_link.py" "$metadata" \
        --check-artifacts "$repo/runtime/$host_target"
else
    echo "abi_exports: runtime metadata not staged for host $host_target" >&2
    exit 1
fi

artifact_dir="$target_dir/$profile"
static_artifact="$artifact_dir/libori_runtime.a"
shared_artifact=""
for candidate in \
    "$artifact_dir/libori_runtime.so" \
    "$artifact_dir/libori_runtime.dylib" \
    "$artifact_dir/ori_runtime.dll"; do
    if [ -f "$candidate" ]; then
        shared_artifact="$candidate"
        break
    fi
done

if [ ! -f "$static_artifact" ]; then
    echo "abi_exports: missing static runtime artifact: $static_artifact" >&2
    exit 1
fi

symbol_tool=""
for candidate in llvm-nm nm; do
    if command -v "$candidate" >/dev/null 2>&1; then
        symbol_tool="$candidate"
        break
    fi
done
if [ -z "$symbol_tool" ]; then
    echo "abi_exports: llvm-nm or nm is required" >&2
    exit 2
fi

expected=$(python3 - "$repo/compiler/crates/ori-types/src/stdlib.rs" \
    "$repo/compiler/crates/ori-codegen/src/native_backend.rs" <<'PY'
import re
import sys

stdlib, backend = (open(path, encoding="utf-8").read() for path in sys.argv[1:])
symbols = set(re.findall(r'stdlib!\([\s\S]*?=>\s*"([^"]+)"[\s\S]*?\)', stdlib))
symbols.update(name for name in re.findall(r'decl\(\s*"([^"]+)"', backend)
                if name.startswith("ori_"))
print("\n".join(sorted(symbols)))
PY
)

exports=$($symbol_tool -g --defined-only "$static_artifact" |
    grep -oE '_?ori_[A-Za-z0-9_]+' |
    sed 's/^_//' | sort -u)

missing=0
for symbol in $expected; do
    if ! printf '%s\n' "$exports" | grep -Fx "$symbol" >/dev/null 2>&1; then
        echo "abi_exports: missing static symbol $symbol" >&2
        missing=1
    fi
done

if [ "$missing" -ne 0 ]; then
    exit 1
fi

if [ -n "$shared_artifact" ]; then
    shared_exports=$($symbol_tool -g --defined-only "$shared_artifact" 2>/dev/null |
        grep -oE '_?ori_[A-Za-z0-9_]+' |
        sed 's/^_//' | sort -u)
    for symbol in ori_rt_init ori_rt_shutdown ori_host_error_code; do
        if ! printf '%s\n' "$shared_exports" | grep -Fx "$symbol" >/dev/null 2>&1; then
            echo "abi_exports: missing shared symbol $symbol" >&2
            missing=1
        fi
    done
else
    echo "abi_exports: shared runtime artifact not found; static checks passed" >&2
fi

if [ "$missing" -ne 0 ]; then
    exit 1
fi

echo "abi_exports: OK ($symbol_tool, $static_artifact${shared_artifact:+, $shared_artifact})"
