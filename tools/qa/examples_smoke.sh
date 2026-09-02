#!/usr/bin/env sh
# Check official examples and optionally compile them — product surface.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
if [ -z "${ORI_BIN:-}" ]; then
  if [ -x "$repo/compiler/target/debug/ori" ]; then
    ORI_BIN="$repo/compiler/target/debug/ori"
  elif [ -x "$repo/compiler/target/release/ori" ]; then
    ORI_BIN="$repo/compiler/target/release/ori"
  else
    ORI_BIN="ori"
  fi
fi
export ORI_USE_SYSTEM_LINKER="${ORI_USE_SYSTEM_LINKER:-1}"

ex_dir="$repo/examples"
if [ ! -d "$ex_dir" ]; then
  echo "no examples/ at $repo" >&2
  exit 0
fi

# Prefer .orl entrypoints / project dirs
fail=0
ok=0
compile_fail=0
compile_ok=0
run_fail=0
run_ok=0
compile_dir=""
run_names=",${ORI_EXAMPLES_RUN:-},"

if [ "${ORI_EXAMPLES_COMPILE:-0}" = "1" ]; then
  compile_dir=$(mktemp -d "${TMPDIR:-/tmp}/ori-examples.XXXXXX")
  trap 'rm -rf "$compile_dir"' EXIT
fi

check_example() {
  entry=$1
  name=$2
  echo "-- check $name --"
  if "$ORI_BIN" check "$entry" >/dev/null 2>&1; then
    ok=$((ok + 1))
    if [ -n "$compile_dir" ]; then
      echo "-- compile $name --"
      compile_output="$compile_dir/$name"
      compile_log="$compile_output.stderr"
      compile_mode="binary"
      compile_args=""
      if grep -Eq '^[[:space:]]*@c_export' "$entry"; then
        # FFI-only examples intentionally have no `main`; validate their
        # shared-library route instead of treating the missing entry point as
        # a compiler failure.
        compile_mode="library"
        compile_args="--lib"
      elif grep -Eq '^[[:space:]]*@test' "$entry" \
        && ! grep -Eq '^[[:space:]]*(pub[[:space:]]+)?main[[:space:]]*[(]' "$entry"; then
        # Test-only projects are compiled by the native test harness, whose
        # generated entry point is deliberately absent from the source.
        compile_mode="test"
      fi
      if [ "$compile_mode" = "test" ]; then
        if "$ORI_BIN" test "$entry" >/dev/null 2>"$compile_log"; then
          compile_rc=0
        else
          compile_rc=$?
        fi
      else
        if "$ORI_BIN" compile "$entry" $compile_args --out "$compile_output" >/dev/null 2>"$compile_log"; then
          compile_rc=0
        else
          compile_rc=$?
        fi
      fi
      if [ "$compile_rc" -eq 0 ]; then
        compile_ok=$((compile_ok + 1))
        if [ "$compile_mode" = "binary" ] && case "$run_names" in *,"$name",*) true;; *) false;; esac; then
          echo "-- run $name --"
          run_log="$compile_output.run.stderr"
          if "$compile_output" >/dev/null 2>"$run_log"; then
            run_ok=$((run_ok + 1))
          else
            echo "FAIL run $entry" >&2
            head -12 "$run_log" >&2 || true
            run_fail=$((run_fail + 1))
          fi
        fi
      else
        echo "FAIL compile $entry" >&2
        head -12 "$compile_log" >&2 || true
        compile_fail=$((compile_fail + 1))
      fi
    fi
  else
    echo "FAIL check $entry" >&2
    "$ORI_BIN" check "$entry" 2>&1 | head -8
    fail=$((fail + 1))
  fi
}

# Root-level examples are valid entrypoints too (for example
# `examples/test_generics.orl`) and were previously skipped by the directory-
# only walk.
for entry in "$ex_dir"/*.orl; do
  [ -f "$entry" ] || continue
  check_example "$entry" "$(basename "$entry")"
done

for f in "$ex_dir"/*/; do
  [ -d "$f" ] || continue
  name=$(basename "$f")
  # skip if no .orl
  if ! ls "$f"*.orl >/dev/null 2>&1 && [ ! -f "$f/ori.pkg.toml" ]; then
    continue
  fi
  entry=""
  if [ -f "$f/main.orl" ]; then
    entry="$f/main.orl"
  else
    for candidate in "$f"*.orl; do
      if [ -f "$candidate" ]; then
        entry="$candidate"
        break
      fi
    done
  fi
  [ -n "$entry" ] || continue
  check_example "$entry" "$name"
done

if [ -n "$compile_dir" ]; then
  echo "examples_smoke: $ok check ok / $fail check fail / $compile_ok build ok / $compile_fail build fail / $run_ok run ok / $run_fail run fail"
  [ "$fail" -eq 0 ] && [ "$compile_fail" -eq 0 ] && [ "$run_fail" -eq 0 ]
else
  echo "examples_smoke: $ok ok / $fail fail"
  [ "$fail" -eq 0 ]
fi
