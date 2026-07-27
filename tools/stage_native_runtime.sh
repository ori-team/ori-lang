#!/usr/bin/env sh
set -eu

target=""
# Default release (LANG-PERF): staged AOT/JIT runtime matches product packages.
# Override with --profile debug or ORI_STAGE_PROFILE=debug for faster rebuild loops.
profile="${ORI_STAGE_PROFILE:-release}"
output_root=""
skip_build=0
skip_bundle_lld=0

usage() {
    cat <<'USAGE'
Usage: tools/stage_native_runtime.sh [--target TRIPLE] [--profile debug|release] [--output-root DIR] [--skip-build] [--skip-bundle-lld]

Stages the Rust ori-runtime static library into:

  runtime/{target-triple}/{runtime-artifact}

and writes runtime-link.json next to it. Also copies rust-lld into
runtime/bin/ unless --skip-bundle-lld is given.

Profile default is **release** (or `$ORI_STAGE_PROFILE`). Use `--profile debug`
when iterating on ori-runtime itself.
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --target)
            target="${2:-}"
            shift 2
            ;;
        --profile)
            profile="${2:-}"
            shift 2
            ;;
        --output-root)
            output_root="${2:-}"
            shift 2
            ;;
        --skip-build)
            skip_build=1
            shift
            ;;
        --skip-bundle-lld)
            skip_bundle_lld=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [ "$profile" != "debug" ] && [ "$profile" != "release" ]; then
    echo "--profile must be debug or release" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)

host_triple() {
    rustc -Vv | awk -F': ' '/^host:/ { print $2; exit }'
}

# Locate rust-lld: ORI_RUST_LLD env -> rustc sysroot -> PATH.
find_rust_lld() {
    if [ -n "${ORI_RUST_LLD:-}" ] && [ -f "$ORI_RUST_LLD" ]; then
        printf '%s\n' "$ORI_RUST_LLD"
        return 0
    fi
    sysroot=$(rustc --print sysroot 2>/dev/null || true)
    if [ -n "$sysroot" ]; then
        host=$(host_triple)
        candidate="$sysroot/lib/rustlib/$host/bin/rust-lld"
        if [ -f "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    fi
    found=$(command -v rust-lld 2>/dev/null || true)
    if [ -n "$found" ]; then
        printf '%s\n' "$found"
        return 0
    fi
    return 1
}

workspace_version() {
    cargo_toml="$repo_root/compiler/Cargo.toml"
    if [ ! -f "$cargo_toml" ]; then
        cargo_toml="$repo_root/Cargo.toml"
    fi
    awk '
        /^\[workspace\.package\]/ { in_section=1; next }
        in_section && /^\[/ { exit }
        in_section && /^[[:space:]]*version[[:space:]]*=/ {
            gsub(/"/, "", $3)
            print $3
            exit
        }
    ' "$cargo_toml"
}

ori_abi_version() {
    sed -n 's/.*pub const ORI_ABI_VERSION[[:space:]]*:[[:space:]]*&str[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
        "$repo_root/compiler/crates/ori-runtime/src/lib.rs" | head -n 1
}

runtime_artifact_name() {
    case "$1" in
        *windows-msvc*) printf '%s\n' "ori_runtime.lib" ;;
        *) printf '%s\n' "libori_runtime.a" ;;
    esac
}

runtime_cdylib_name() {
    case "$1" in
        *windows-msvc*) printf '%s\n' "ori_runtime.dll" ;;
        *apple-darwin*) printf '%s\n' "libori_runtime.dylib" ;;
        *) printf '%s\n' "libori_runtime.so" ;;
    esac
}

fallback_native_static_libs() {
    case "$1" in
        *linux*) printf '%s\n' "-lpthread -ldl -lm -no-pie" ;;
        *apple-darwin*) printf '%s\n' "System" ;;
        *windows-msvc*) printf '%s\n' "legacy_stdio_definitions.lib bcrypt.lib kernel32.lib ntdll.lib userenv.lib ws2_32.lib dbghelp.lib /defaultlib:msvcrt" ;;
        *) printf '\n' ;;
    esac
}

required_native_link_args() {
    target_triple="$1"
    libs="$2"
    case "$target_triple" in
        *linux*)
            case " $libs " in
                *" -no-pie "*) printf '%s\n' "$libs" ;;
                *) printf '%s -no-pie\n' "$libs" ;;
            esac
            ;;
        *) printf '%s\n' "$libs" ;;
    esac
}

native_static_libs() {
    profile_args=""
    if [ "$profile" = "release" ]; then
        profile_args="--release"
    fi

    set +e
    output=$(cd "$repo_root/compiler" && cargo rustc -p ori-runtime --lib --target "$target" $profile_args -- --print native-static-libs 2>&1)
    status=$?
    set -e
    if [ "$status" -eq 0 ]; then
        libs=$(printf '%s\n' "$output" | sed -n 's/.*native-static-libs:[[:space:]]*//p' | tail -n 1)
        if [ -n "$libs" ]; then
            printf '%s\n' "$libs"
            return
        fi
    fi

    fallback_native_static_libs "$target"
}

json_array_from_words() {
    first=1
    printf '['
    for item in $1; do
        if [ "$first" -eq 0 ]; then
            printf ', '
        fi
        first=0
        escaped=$(printf '%s' "$item" | sed 's/\\/\\\\/g; s/"/\\"/g')
        printf '"%s"' "$escaped"
    done
    printf ']'
}

if [ -z "$target" ]; then
    target=$(host_triple)
fi
if [ -z "$target" ]; then
    echo "could not detect Rust host target; pass --target explicitly" >&2
    exit 2
fi

ori_version=$(workspace_version)
abi_version=$(ori_abi_version)
if [ -z "$ori_version" ]; then
    echo "could not read workspace package version from Cargo.toml" >&2
    exit 2
fi
if [ -z "$abi_version" ]; then
    echo "could not read ORI_ABI_VERSION from ori-runtime" >&2
    exit 2
fi

artifact=$(runtime_artifact_name "$target")
cdylib_artifact=$(runtime_cdylib_name "$target")
profile_args=""
if [ "$profile" = "release" ]; then
    profile_args="--release"
fi

if [ "$skip_build" -eq 0 ]; then
    (cd "$repo_root/compiler" && cargo build -p ori-runtime --lib --target "$target" $profile_args)
fi

target_root="${CARGO_TARGET_DIR:-$repo_root/compiler/target}"
source="$target_root/$target/$profile/$artifact"
if [ ! -f "$source" ]; then
    source="$target_root/$profile/$artifact"
fi
# Host-triple builds without --target land under profile/ only.
if [ ! -f "$source" ]; then
    source="$target_root/$profile/$artifact"
fi
if [ ! -f "$source" ]; then
    echo "Runtime artifact $artifact was not found after build under $target_root." >&2
    exit 1
fi

cdylib_source="$target_root/$target/$profile/$cdylib_artifact"
if [ ! -f "$cdylib_source" ]; then
    cdylib_source="$target_root/$profile/$cdylib_artifact"
fi
cdylib_found=0
if [ -f "$cdylib_source" ]; then
    cdylib_found=1
fi

if [ -z "$output_root" ]; then
    stage_root="$repo_root/runtime"
else
    stage_root="$output_root"
fi
target_dir="$stage_root/$target"
mkdir -p "$target_dir"

dest="$target_dir/$artifact"
cp "$source" "$dest"

if [ "$cdylib_found" -eq 1 ]; then
    cdylib_dest="$target_dir/$cdylib_artifact"
    cp "$cdylib_source" "$cdylib_dest"
    printf 'staged runtime cdylib: %s\n' "$cdylib_dest"
else
    echo "warning: runtime cdylib $cdylib_artifact was not found after build; JIT mode (ORI_USE_JIT=1) will not be available." >&2
fi

cdylib_value="$cdylib_artifact"
if [ "$cdylib_found" -ne 1 ]; then
    cdylib_value=""
fi

libs=$(required_native_link_args "$target" "$(native_static_libs)")
libs_json=$(json_array_from_words "$libs")
metadata_path="$target_dir/runtime-link.json"
cat > "$metadata_path" <<JSON
{
  "target": "$target",
  "runtime": "$artifact",
  "runtime_cdylib": "$cdylib_value",
  "ori_version": "$ori_version",
  "abi_version": "$abi_version",
  "profile": "$profile",
  "native_static_libs": $libs_json,
  "generated_by": "tools/stage_native_runtime.sh"
}
JSON

printf 'staged runtime: %s\n' "$dest"
printf 'metadata: %s\n' "$metadata_path"

if [ "$skip_bundle_lld" -eq 0 ]; then
    if lld_path=$(find_rust_lld); then
        bin_dir="$stage_root/bin"
        mkdir -p "$bin_dir"
        lld_dest="$bin_dir/$(basename "$lld_path")"
        cp "$lld_path" "$lld_dest"
        printf 'staged rust-lld: %s\n' "$lld_dest"
    else
        echo "warning: rust-lld not found; ORI_USE_BUNDLED_RUST_LLD will require ORI_RUST_LLD or rustc sysroot at link time." >&2
    fi
fi
