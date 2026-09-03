#!/usr/bin/env sh
# Ori language QA — full daily/weekly (S0–S8).
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
"$script_dir/daily_fast.sh"

repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
if [ -f "$repo/compiler/Cargo.toml" ]; then
  comp="$repo/compiler"
elif [ -f "$repo/Cargo.toml" ]; then
  comp="$repo"
else
  echo "workspace not found" >&2
  exit 2
fi
cd "$comp"

echo "== S4 multifile_imports =="
cargo test -p ori-driver --test multifile_imports --locked -- --quiet

echo "== S3 concurrency_async full =="
cargo test -p ori-driver --test concurrency_async --locked -- --quiet

echo "== S5 cargo test --workspace =="
RUST_TEST_THREADS=1 cargo test --workspace --locked -- --quiet

echo "== S6 examples smoke =="
if [ -x "$script_dir/examples_smoke.sh" ]; then
  ORI_EXAMPLES_COMPILE=1 \
    ORI_EXAMPLES_RUN="${ORI_EXAMPLES_RUN:-hello,language_features,native_showcase,collections_demo,string_toolkit,bytes_usage,conditional_config,error_handling}" \
    "$script_dir/examples_smoke.sh"
fi

observational_incomplete=0
echo "== S6a front-end hostile-input smoke =="
if [ -x "$script_dir/fuzz_smoke.py" ]; then
  if ! python3 "$script_dir/fuzz_smoke.py"; then
    echo "daily_full: WARNING/INCOMPLETE — front-end fuzz smoke unavailable or failed" >&2
    observational_incomplete=1
  fi
fi

echo "== S6b packages web SEC8 =="
if [ -x "$script_dir/web_sec8.sh" ] && [ -d "$repo/packages/ori-web/examples/sec8_tests" ]; then
  if ! "$script_dir/web_sec8.sh"; then
    echo "daily_full: WARNING/INCOMPLETE — web SEC8 smoke failed" >&2
    observational_incomplete=1
  fi
else
  echo "daily_full: WARNING/INCOMPLETE — web SEC8 fixture unavailable" >&2
  observational_incomplete=1
fi

echo "== S6c packages web_auth smoke =="
if [ -x "$script_dir/web_auth_smoke.sh" ] && [ -d "$repo/packages/ori-web-auth/examples/smoke" ]; then
  if ! "$script_dir/web_auth_smoke.sh"; then
    echo "daily_full: WARNING/INCOMPLETE — web_auth smoke failed" >&2
    observational_incomplete=1
  fi
else
  echo "daily_full: WARNING/INCOMPLETE — web_auth fixture unavailable" >&2
  observational_incomplete=1
fi

echo "== S6d packages web_session_sqlite smoke =="
if [ -x "$script_dir/web_session_sqlite_smoke.sh" ] && [ -d "$repo/packages/ori-web-session-sqlite/examples/smoke" ]; then
  if ! "$script_dir/web_session_sqlite_smoke.sh"; then
    echo "daily_full: WARNING/INCOMPLETE — web_session_sqlite smoke failed" >&2
    observational_incomplete=1
  fi
else
  echo "daily_full: WARNING/INCOMPLETE — web_session_sqlite fixture unavailable" >&2
  observational_incomplete=1
fi

echo "== S7 perf =="
if [ -x "$script_dir/perf_daily.sh" ]; then
  if ! "$script_dir/perf_daily.sh"; then
    echo "daily_full: WARNING/INCOMPLETE — observational performance stage failed" >&2
    observational_incomplete=1
  fi
fi

if [ "$observational_incomplete" -ne 0 ]; then
  echo "daily_full: REQUIRED GATES PASSED; observational stages INCOMPLETE" >&2
else
  echo "daily_full: OK"
fi
