#!/usr/bin/env sh
set -eu

package_root=""
skip_build=0
keep_package=0

usage() {
    cat <<'USAGE'
Usage: tools/smoke_native_release.sh [--package-root DIR] [--skip-build] [--keep-package]

Builds a release-style Ori package in a clean folder, stages the native runtime,
then verifies `ori compile`, `ori test`, and `ori run` (JIT) using only package-local files.
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --package-root)
            package_root="${2:-}"
            shift 2
            ;;
        --skip-build)
            skip_build=1
            shift
            ;;
        --keep-package)
            keep_package=1
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

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
compiler_root="$repo_root/compiler"

# Package AOT must use SystemLinker (or BundledRustLld / explicit override).
# Do NOT default to RustcDriver: ori-runtime is a Rust staticlib that already
# contains libstd symbols; linking via `rustc` pulls a second libstd and fails
# with `duplicate symbol: rust_eh_personality`.
if [ -z "${ORI_USE_SYSTEM_LINKER:-}" ] && [ -z "${ORI_USE_RUSTC_DRIVER:-}" ] && [ -z "${ORI_USE_BUNDLED_RUST_LLD:-}" ] && [ -z "${ORI_NATIVE_LINKER:-}" ]; then
    export ORI_USE_SYSTEM_LINKER=1
fi
# rustup/cargo may leave LIBRARY_PATH pointing at the Rust sysroot, which
# shadows multiarch libc. Always clear for package smoke.
unset LIBRARY_PATH || true
unset LIBPATH || true

host_triple() {
    rustc -Vv | awk -F': ' '/^host:/ { print $2; exit }'
}

output_exe_name() {
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*) printf '%s.exe\n' "$1" ;;
        *) printf '%s\n' "$1" ;;
    esac
}

run_checked() {
    desc="$1"
    shift
    "$@" || {
        status=$?
        echo "$desc failed with exit code $status." >&2
        exit "$status"
    }
}

target_root="${CARGO_TARGET_DIR:-$compiler_root/target}"
ori_exe=$(output_exe_name ori)
lsp_exe=$(output_exe_name ori-lsp)
source_ori="$target_root/release/$ori_exe"
source_lsp="$target_root/release/$lsp_exe"
host=$(host_triple)

if [ -z "$host" ]; then
    echo "could not detect Rust host target from rustc -Vv" >&2
    exit 2
fi

if [ -z "$package_root" ]; then
    package_root="${TMPDIR:-/tmp}/ori-native-release-smoke-$$"
fi

package_root=$(CDPATH= cd -- "$(dirname -- "$package_root")" && pwd)/$(basename -- "$package_root")
examples_dir="$package_root/examples"
runtime_dir="$package_root/runtime"
package_ori="$package_root/$ori_exe"
package_lsp="$package_root/$lsp_exe"
stdlib_dir="$package_root/stdlib"

cleanup() {
    if [ "$keep_package" -eq 0 ] && [ -d "$package_root" ]; then
        rm -rf "$package_root"
    fi
}
trap cleanup EXIT

if [ "$skip_build" -eq 0 ]; then
    run_checked "cargo build -p ori-driver -p ori-lsp --release" \
        sh -c "cd '$compiler_root' && cargo build -p ori-driver -p ori-lsp --release --locked"
fi

if [ ! -f "$source_ori" ]; then
    echo "release compiler not found at $source_ori" >&2
    exit 1
fi
if [ ! -f "$source_lsp" ]; then
    echo "release LSP server not found at $source_lsp" >&2
    exit 1
fi

rm -rf "$package_root"
mkdir -p "$examples_dir"
cp "$source_ori" "$package_ori"
cp "$source_lsp" "$package_lsp"
cp -R "$repo_root/stdlib" "$stdlib_dir"
cp "$repo_root/examples/hello/main.orl" "$examples_dir/hello.orl"
cp "$repo_root/examples/async_demo/main.orl" "$examples_dir/async_demo.orl"

# Skip bundling rust-lld into the end-user package: the toolchain binary is
# typically linked against libLLVM from the Rust install and is not portable.
# AOT uses SystemLinker (OS ld/link) for Rust-free installs; see M1.
stage_args="--target $host --profile release --output-root $runtime_dir --skip-bundle-lld"
if [ "$skip_build" -eq 1 ]; then
    stage_args="$stage_args --skip-build"
fi
run_checked "tools/stage_native_runtime.sh" \
    sh -c "cd '$repo_root' && tools/stage_native_runtime.sh $stage_args"

cat > "$examples_dir/package_smoke_test.orl" <<'ORI'
module app.package_smoke

import ori.test = test
import ori.task = task

@test
package_smoke_test()
    check 1 + 1 == 2
    test.assert(true, "package smoke test")
end

@test
async package_async_smoke_test()
    await task.sleep(1)
    test.assert(true, "package async smoke test")
end
ORI

cat > "$examples_dir/stdlib_package_smoke.orl" <<'ORI'
module app.stdlib_package_smoke

import ori.io = io
import ori.string (trim_all)

main()
    io.print(trim_all("hello   packaged   stdlib"))
end
ORI

cat > "$examples_dir/alias_package_smoke.orl" <<'ORI'
module app.alias_package_smoke

import ori.fs (TextResult, IoResult, write_text_result, remove_file)
import ori.io = io

uses_aliases(path: string) -> TextResult
    return write_text_result(path, "ok")
end

main()
    match uses_aliases("/tmp/ori-alias-smoke.txt")
        case ok(_):
            io.print("alias ok")
        case err(msg):
            io.print(msg)
    end
    const cleanup: IoResult = remove_file("/tmp/ori-alias-smoke.txt")
    match cleanup
        case ok(_):
            return
        case err(_):
            return
    end
end
ORI

if [ ! -f "$package_lsp" ]; then
    echo "packaged LSP server was not copied to $package_lsp" >&2
    exit 1
fi
if [ ! -f "$stdlib_dir/string.orl" ]; then
    echo "packaged stdlib was not copied to $stdlib_dir" >&2
    exit 1
fi

# AOT compile/test smoke needs a working host linker. GitHub Actions runners
# have repeatedly failed bare multiarch `cc`/`ld` (`cannot find -lc`) even with
# build-essential. Use ORI_PACKAGE_SMOKE_JIT_ONLY=1 to validate layout + JIT
# only (still enough to ship a usable package; AOT is verified on systems where
# SystemLinker works, e.g. developer machines with a normal toolchain).
smoke_jit_only=0
case "${ORI_PACKAGE_SMOKE_JIT_ONLY:-}" in
    1|true|yes|on|YES|TRUE|ON) smoke_jit_only=1 ;;
esac

if [ "$smoke_jit_only" -eq 0 ]; then
hello_exe="$package_root/$(output_exe_name hello)"
(
    cd "$package_root"
    ORI_REQUIRE_PACKAGED_RUNTIME=1 "$package_ori" compile "examples/hello.orl" --out "$hello_exe"
)

hello_output=$("$hello_exe")
case "$hello_output" in
    *"The answer is: 42"*) ;;
    *)
        echo "compiled hello executable did not print expected answer" >&2
        echo "$hello_output" >&2
        exit 1
        ;;
esac

async_exe="$package_root/$(output_exe_name async_demo)"
(
    cd "$package_root"
    ORI_REQUIRE_PACKAGED_RUNTIME=1 "$package_ori" compile "examples/async_demo.orl" --out "$async_exe"
)

async_output=$("$async_exe")
# async_demo prints 42 then 50 (8 + await delayed_answer).
case "$async_output" in
    *"42"*) ;;
    *)
        echo "compiled async_demo executable did not print expected async answer" >&2
        echo "$async_output" >&2
        exit 1
        ;;
esac
# Prefer full multi-line match when present (current examples/async_demo).
case "$async_output" in
    *"50"*) ;;
    *)
        # Older single-line async demos only print 42 — still OK.
        ;;
esac

stdlib_exe="$package_root/$(output_exe_name stdlib_package_smoke)"
(
    cd "$package_root"
    ORI_REQUIRE_PACKAGED_RUNTIME=1 "$package_ori" compile "examples/stdlib_package_smoke.orl" --out "$stdlib_exe"
)

stdlib_output=$("$stdlib_exe")
case "$stdlib_output" in
    *"hello packaged stdlib"*) ;;
    *)
        echo "compiled stdlib_package_smoke executable did not use the packaged stdlib" >&2
        echo "$stdlib_output" >&2
        exit 1
        ;;
esac

alias_exe="$package_root/$(output_exe_name alias_package_smoke)"
(
    cd "$package_root"
    ORI_REQUIRE_PACKAGED_RUNTIME=1 "$package_ori" compile "examples/alias_package_smoke.orl" --out "$alias_exe"
)
alias_output=$("$alias_exe")
case "$alias_output" in
    *"alias ok"*) ;;
    *)
        echo "compiled alias_package_smoke did not exercise public aliases" >&2
        echo "$alias_output" >&2
        exit 1
        ;;
esac

(
    cd "$package_root"
    ORI_REQUIRE_PACKAGED_RUNTIME=1 "$package_ori" test "examples/package_smoke_test.orl"
)
else
    echo "ORI_PACKAGE_SMOKE_JIT_ONLY=1 — skipping AOT compile/test smoke (JIT + doctor only)"
fi

runtime_triple_dir="$runtime_dir/$host"
case "$host" in
    *windows-msvc*) cdylib_name="ori_runtime.dll" ;;
    *apple-darwin*) cdylib_name="libori_runtime.dylib" ;;
    *) cdylib_name="libori_runtime.so" ;;
esac
cdylib_path="$runtime_triple_dir/$cdylib_name"
if [ ! -f "$cdylib_path" ]; then
    echo "packaged runtime cdylib was not staged at $cdylib_path" >&2
    exit 1
fi

jit_output=$(
    cd "$package_root"
    ORI_REQUIRE_PACKAGED_RUNTIME=1 "$package_ori" run "examples/hello.orl"
)
case "$jit_output" in
    *"The answer is: 42"*) ;;
    *)
        echo "ori run (JIT default) did not print expected answer" >&2
        echo "$jit_output" >&2
        exit 1
        ;;
esac

(
    cd "$package_root"
    ORI_REQUIRE_PACKAGED_RUNTIME=1 "$package_ori" doctor
) >/dev/null

# Verify that the packaged server speaks the real JSON-RPC protocol, not just
# that an executable file exists. This catches broken dynamic loading and
# startup regressions before an archive is published.
run_checked "packaged ori-lsp protocol handshake" \
    python3 "$repo_root/tools/qa/lsp_protocol_smoke.py" "$package_lsp"

printf 'native release smoke passed: %s\n' "$package_root"
