#!/usr/bin/env sh
# Smoke for packages/ori-web-session-sqlite (requires ori-sqlite native build).
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)

if [ -x "$repo/compiler/target/debug/ori" ]; then
  ORI_BIN="${ORI_BIN:-$repo/compiler/target/debug/ori}"
elif [ -x "$repo/compiler/target/release/ori" ]; then
  ORI_BIN="${ORI_BIN:-$repo/compiler/target/release/ori}"
else
  ORI_BIN="${ORI_BIN:-ori}"
fi

export ORI_STDLIB_ROOT="${ORI_STDLIB_ROOT:-$repo/stdlib}"
export ORI_USE_SYSTEM_LINKER="${ORI_USE_SYSTEM_LINKER:-1}"
# Static native package libs (sqlite) need AOT link, not JIT cdylib resolution.
export ORI_USE_AOT=1
if [ -f "$repo/runtime/x86_64-unknown-linux-gnu/libori_runtime.a" ]; then
  export ORI_RUNTIME_LIB="${ORI_RUNTIME_LIB:-$repo/runtime/x86_64-unknown-linux-gnu/libori_runtime.a}"
fi

# Prefer monorepo symlink packages/ori-sqlite, then ORI_SQLITE_ROOT, then Documentos default.
smoke_root="$repo/packages/ori-web-session-sqlite/examples/smoke"
if [ ! -d "$smoke_root" ]; then
  echo "web_session_sqlite_smoke: SKIP (package fixture is not present)" >&2
  exit 0
fi
if [ -n "${ORI_SQLITE_ROOT:-}" ]; then
  SQLITE_ROOT="$ORI_SQLITE_ROOT"
elif [ -d "$repo/packages/ori-sqlite" ]; then
  SQLITE_ROOT="$repo/packages/ori-sqlite"
else
  SQLITE_ROOT="$HOME/Documentos/Projetos/ori-sqlite"
fi
# Ensure packages/ori-sqlite exists for relative path deps in ori.pkg.toml
if [ ! -e "$repo/packages/ori-sqlite" ] && [ -d "$SQLITE_ROOT" ]; then
  ln -sfn "$SQLITE_ROOT" "$repo/packages/ori-sqlite"
fi
if [ ! -f "$SQLITE_ROOT/lib/x86_64-unknown-linux-gnu/libsqlite3.a" ]; then
  if [ -x "$SQLITE_ROOT/tools/build_linux.sh" ]; then
    echo "== build ori-sqlite native ($SQLITE_ROOT) =="
    "$SQLITE_ROOT/tools/build_linux.sh"
  else
    echo "web_session_sqlite_smoke: SKIP (ori-sqlite not built at $SQLITE_ROOT)" >&2
    exit 0
  fi
fi

package_cache=$(mktemp -d "${TMPDIR:-/tmp}/ori-web-session-cache.XXXXXX")
trap 'rm -rf -- "$package_cache"' EXIT HUP INT TERM
export ORI_PACKAGE_CACHE="$package_cache"
cd "$smoke_root"

echo "== web_session_sqlite_smoke: $ORI_BIN run main.orl (AOT) =="
out=$("$ORI_BIN" run main.orl 2>&1) || {
  echo "$out" >&2
  echo "web_session_sqlite_smoke: FAIL (run error)" >&2
  exit 1
}
echo "$out"
echo "$out" | grep -q "SQLITE_SESSION SMOKE PASSED" || {
  echo "web_session_sqlite_smoke: FAIL (missing SQLITE_SESSION SMOKE PASSED)" >&2
  exit 1
}
echo "web_session_sqlite_smoke: OK"
