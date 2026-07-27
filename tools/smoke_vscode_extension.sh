#!/usr/bin/env sh
set -eu

workspace_root=""
skip_cargo_build=0
skip_npm_install=0
skip_lsp_e2e=0
keep_workspace=0

usage() {
    cat <<'USAGE'
Usage: tools/smoke_vscode_extension.sh [--workspace-root DIR] [--skip-cargo-build] [--skip-npm-install] [--skip-lsp-e2e] [--keep-workspace]

Compiles the VS Code extension, validates extension JSON files, runs the LSP E2E
suite, and checks a temporary Ori project outside the repository.
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --workspace-root)
            workspace_root="${2:-}"
            shift 2
            ;;
        --skip-cargo-build)
            skip_cargo_build=1
            shift
            ;;
        --skip-npm-install)
            skip_npm_install=1
            shift
            ;;
        --skip-lsp-e2e)
            skip_lsp_e2e=1
            shift
            ;;
        --keep-workspace)
            keep_workspace=1
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
extension_root="$repo_root/extensions/vscode-orl"
# Workspace lives under compiler/; prefer that target unless CARGO_TARGET_DIR is set.
target_root="${CARGO_TARGET_DIR:-$repo_root/compiler/target}"

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

assert_smoke_root() {
    full=$(CDPATH= cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")
    case "$(basename -- "$full")" in
        ori-vscode-extension-smoke-*) printf '%s\n' "$full" ;;
        *)
            echo "refusing to remove smoke workspace with unexpected name: $full" >&2
            exit 2
            ;;
    esac
}

assert_json_file() {
    node -e "JSON.parse(require('fs').readFileSync(process.argv[1], 'utf8'))" "$1"
}

if [ -z "$workspace_root" ]; then
    workspace_root="${TMPDIR:-/tmp}/ori-vscode-extension-smoke-$$"
fi
workspace_root=$(assert_smoke_root "$workspace_root")
project_root="$workspace_root/demo"
project_entry="$project_root/main.orl"
ori_exe="$target_root/debug/$(output_exe_name ori)"
lsp_exe="$target_root/debug/$(output_exe_name ori-lsp)"

cleanup() {
    if [ "$keep_workspace" -eq 0 ] && [ -d "$workspace_root" ]; then
        rm -rf "$workspace_root"
    fi
}
trap cleanup EXIT

if [ "$skip_cargo_build" -eq 0 ]; then
    run_checked "cargo build -p ori-driver -p ori-lsp" \
        sh -c "cd '$repo_root/compiler' && cargo build -p ori-driver -p ori-lsp"
fi

if [ ! -f "$ori_exe" ]; then
    echo "Ori compiler was not found at $ori_exe" >&2
    exit 1
fi
if [ ! -f "$lsp_exe" ]; then
    echo "Ori LSP server was not found at $lsp_exe" >&2
    exit 1
fi

if [ "$skip_npm_install" -eq 0 ] && [ ! -d "$extension_root/node_modules" ]; then
    run_checked "npm install" sh -c "cd '$extension_root' && npm install"
fi
run_checked "npm run package:vsix" sh -c "cd '$extension_root' && npm run package:vsix"
assert_json_file "$extension_root/package.json"
assert_json_file "$extension_root/language-configuration.json"
assert_json_file "$extension_root/oridoc-language-configuration.json"
assert_json_file "$extension_root/snippets/ori.json"
assert_json_file "$extension_root/syntaxes/ori.tmLanguage.json"
assert_json_file "$extension_root/syntaxes/oridoc.tmLanguage.json"

extension_version=$(node -e "console.log(require('$extension_root/package.json').version)")
vsix_path="$extension_root/vscode-orl-${extension_version}.vsix"
if [ ! -f "$vsix_path" ]; then
    echo "VSIX was not found at $vsix_path" >&2
    exit 1
fi
if ! unzip -Z1 "$vsix_path" | grep -Fqx \
    "extension/node_modules/vscode-languageclient/node.js"; then
    echo "VSIX is missing the vscode-languageclient runtime dependency" >&2
    exit 1
fi

if [ "$skip_lsp_e2e" -eq 0 ]; then
    run_checked "cargo test -p ori-lsp --test e2e" \
        sh -c "cd '$repo_root/compiler' && cargo test -p ori-lsp --test e2e"
fi

rm -rf "$workspace_root"
mkdir -p "$workspace_root"
run_checked "ori new outside repository" "$ori_exe" new "$project_root" --name vscode_smoke

mkdir -p "$project_root/.vscode"
cat > "$project_root/.vscode/settings.json" <<EOF
{
  "ori.lsp.path": "$lsp_exe",
  "ori.compiler.path": "$ori_exe",
  "ori.stdlib.root": "$repo_root/stdlib"
}
EOF

run_checked "ori check outside repository" "$ori_exe" check "$project_root/ori.proj"
run_checked "ori run outside repository" "$ori_exe" run "$project_entry"
run_checked "ori test outside repository" "$ori_exe" test "$project_entry"
run_checked "ori doc check outside repository" "$ori_exe" doc check "$project_root/ori.proj"
run_checked "ori summary outside repository" "$ori_exe" summary "$project_root/ori.proj"
run_checked "ori fmt outside repository" "$ori_exe" fmt "$project_entry"

printf 'VS Code extension smoke passed: %s\n' "$workspace_root"
