#!/usr/bin/env sh
# Polyglot runtime microbench: Ori AOT vs many languages.
# Languages: ori, python, rust, c, go, javascript, typescript, ruby, nim
# Timer: Python time.perf_counter (subprocess wall time).
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# Linker env can leak from the parent shell and break AOT (e.g. force SystemLinker
# without multiarch -L). Prefer product defaults unless the user set them.
# shellcheck disable=SC2034
: "${ORI_USE_SYSTEM_LINKER:=}"
ORI_BIN="${ORI_BIN:-$(command -v ori || true)}"
PYTHON="${PYTHON:-python3}"
NODE="${NODE:-$(command -v node || true)}"
TSC="${TSC:-$(command -v tsc || true)}"
RUBY="${RUBY:-$(command -v ruby || true)}"
GO="${GO:-$(command -v go || true)}"
CC="${CC:-$(command -v gcc || command -v cc || true)}"
NIM="${NIM:-$(command -v nim || true)}"
samples="${SAMPLES:-5}"
out_dir="$root/results"
mkdir -p "$out_dir" "$root/bin" "$root/bin/c" "$root/bin/go" "$root/bin/ts" "$root/bin/nim"
stamp=$(date -Iseconds 2>/dev/null || date)
report="$out_dir/report_$(date +%Y%m%d_%H%M%S).md"
: >"$out_dir/compile_times.txt"

workloads="sum_loop fib_iter list_sum nested vec4_simd arena_bulk_alloc channel_throughput spatial_grid_bvh"

# language id → display name
langs_all="ori python rust c go javascript typescript ruby nim"

median() {
  sort -n | awk '
    { a[NR]=$1 }
    END {
      if (NR==0) { print "nan"; exit }
      if (NR%2==1) printf "%.6f", a[(NR+1)/2]
      else printf "%.6f", (a[NR/2]+a[NR/2+1])/2
    }'
}

time_cmd() {
  out_file=$1
  shift
  "$PYTHON" - "$out_file" "$@" <<'PY'
import subprocess, sys, time
out_path = sys.argv[1]
cmd = sys.argv[2:]
t0 = time.perf_counter()
with open(out_path, "w", encoding="utf-8") as f:
    r = subprocess.run(cmd, stdout=f, stderr=subprocess.PIPE, text=True)
t1 = time.perf_counter()
if r.returncode != 0:
    sys.stderr.write(r.stderr or "")
    sys.exit(r.returncode)
print(f"{t1 - t0:.6f}")
PY
}

have() {
  command -v "$1" >/dev/null 2>&1
}

echo "# Polyglot bench report" >"$report"
echo "" >>"$report"
echo "- **When:** $stamp" >>"$report"
echo "- **Host:** $(uname -srm)" >>"$report"
echo "- **CPU:** $(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | sed 's/^ //' || echo unknown)" >>"$report"
echo "- **samples per cell:** $samples" >>"$report"
echo "- **timer:** Python \`time.perf_counter\` (subprocess wall time)" >>"$report"
echo "" >>"$report"
echo "## Toolchain versions" >>"$report"
echo "" >>"$report"
echo "| Lang | Version |" >>"$report"
echo "|------|---------|" >>"$report"

version_line() {
  lang=$1
  ver=$2
  echo "| $lang | $ver |" >>"$report"
  echo "  $lang: $ver"
}

active_langs=""

# --- detect / record versions ---
if [ -n "$ORI_BIN" ] && [ -x "$ORI_BIN" ] || have ori; then
  ORI_BIN="${ORI_BIN:-ori}"
  version_line ori "$($ORI_BIN --version 2>/dev/null || echo ori)"
  active_langs="$active_langs ori"
else
  echo "SKIP ori (not found)" >&2
fi

version_line python "$($PYTHON --version 2>&1)"
active_langs="$active_langs python"

if have rustc && have cargo; then
  version_line rust "$(rustc --version)"
  active_langs="$active_langs rust"
else
  echo "SKIP rust" >&2
fi

if [ -n "$CC" ]; then
  version_line c "$($CC --version 2>&1 | head -1)"
  active_langs="$active_langs c"
else
  echo "SKIP c" >&2
fi

if [ -n "$GO" ]; then
  version_line go "$($GO version 2>&1)"
  active_langs="$active_langs go"
else
  echo "SKIP go" >&2
fi

if [ -n "$NODE" ]; then
  version_line javascript "node $($NODE -v 2>&1)"
  active_langs="$active_langs javascript"
else
  echo "SKIP javascript" >&2
fi

if [ -n "$TSC" ] && [ -n "$NODE" ]; then
  version_line typescript "$($TSC -v 2>&1) / node $($NODE -v 2>&1)"
  active_langs="$active_langs typescript"
else
  echo "SKIP typescript (need tsc + node)" >&2
fi

if [ -n "$RUBY" ]; then
  version_line ruby "$($RUBY -v 2>&1)"
  active_langs="$active_langs ruby"
else
  echo "SKIP ruby" >&2
fi

if [ -n "$NIM" ]; then
  version_line nim "$($NIM --version 2>&1 | head -1)"
  active_langs="$active_langs nim"
else
  echo "SKIP nim" >&2
fi

echo "" >>"$report"
echo "## Workloads" >>"$report"
echo "" >>"$report"
echo "| Workload | Description |" >>"$report"
echo "|----------|-------------|" >>"$report"
echo "| sum_loop | sum 0..10_000_000-1 |" >>"$report"
echo "| fib_iter | iterative fib **20_000_000 steps** (i64 wrap where needed) |" >>"$report"
echo "| list_sum | push 1_000_000 ints + sum |" >>"$report"
echo "| nested | nested loops 2000×2000 |" >>"$report"
echo "| vec4_simd | 5_000_000 4D vector additions (SIMD vs scalar) |" >>"$report"
echo "| arena_bulk_alloc | 100_000 frame bump-arena resets (O(1) vs GC) |" >>"$report"
echo "| channel_throughput | 100_000 message send/recv in bounded channel |" >>"$report"
echo "| spatial_grid_bvh | 1_000_000 @align(16) AABB intersection tests |" >>"$report"
echo "" >>"$report"

echo "== compile phase =="

# Rust
if echo "$active_langs" | grep -qw rust; then
  for w in $workloads; do
    (cd "$root/rust_$w" && cargo build --release -q)
  done
fi

# Ori (compile via /tmp then install — avoids stale .o next to multi-artifact bin/)
if echo "$active_langs" | grep -qw ori; then
  for w in $workloads; do
    src="$root/ori/${w}.orl"
    bin="$root/bin/ori_${w}"
    tmpbin="/tmp/ori_polyglot_${w}_$$"
    echo "  ori compile $w"
    t0=$("$PYTHON" -c 'import time; print(time.perf_counter())')
    "$ORI_BIN" compile "$src" --out "$tmpbin" >/dev/null
    mv -f "$tmpbin" "$bin"
    chmod +x "$bin"
    t1=$("$PYTHON" -c 'import time; print(time.perf_counter())')
    ct=$("$PYTHON" -c "print(f'{$t1 - $t0:.3f}')")
    echo "ori_compile_${w}=${ct}" >>"$out_dir/compile_times.txt"
    echo "  ori $w compile ${ct}s"
  done
fi

# C -O2
if echo "$active_langs" | grep -qw c; then
  for w in $workloads; do
    echo "  gcc -O2 $w"
    t0=$("$PYTHON" -c 'import time; print(time.perf_counter())')
    "$CC" -O2 -std=c11 "$root/c/${w}.c" -o "$root/bin/c/${w}"
    t1=$("$PYTHON" -c 'import time; print(time.perf_counter())')
    ct=$("$PYTHON" -c "print(f'{$t1 - $t0:.3f}')")
    echo "c_compile_${w}=${ct}" >>"$out_dir/compile_times.txt"
  done
fi

# Go
if echo "$active_langs" | grep -qw go; then
  for w in $workloads; do
    echo "  go build $w"
    t0=$("$PYTHON" -c 'import time; print(time.perf_counter())')
    (cd "$root/go" && "$GO" build -o "$root/bin/go/${w}" "${w}.go")
    t1=$("$PYTHON" -c 'import time; print(time.perf_counter())')
    ct=$("$PYTHON" -c "print(f'{$t1 - $t0:.3f}')")
    echo "go_compile_${w}=${ct}" >>"$out_dir/compile_times.txt"
  done
fi

# TypeScript (per-file so top-level consts do not collide)
if echo "$active_langs" | grep -qw typescript; then
  echo "  tsc (per file)"
  t0=$("$PYTHON" -c 'import time; print(time.perf_counter())')
  for w in $workloads; do
    "$TSC" "$root/typescript/${w}.ts" \
      --outDir "$root/bin/ts" \
      --target ES2020 \
      --module commonjs \
      --strict \
      --esModuleInterop \
      --skipLibCheck
  done
  t1=$("$PYTHON" -c 'import time; print(time.perf_counter())')
  ct=$("$PYTHON" -c "print(f'{$t1 - $t0:.3f}')")
  echo "ts_compile_all=${ct}" >>"$out_dir/compile_times.txt"
fi

# Nim -d:release
if echo "$active_langs" | grep -qw nim; then
  for w in $workloads; do
    echo "  nim c -d:release $w"
    t0=$("$PYTHON" -c 'import time; print(time.perf_counter())')
    "$NIM" c -d:release --hints:off --warnings:off -o:"$root/bin/nim/${w}" "$root/nim/${w}.nim" >/dev/null
    t1=$("$PYTHON" -c 'import time; print(time.perf_counter())')
    ct=$("$PYTHON" -c "print(f'{$t1 - $t0:.3f}')")
    echo "nim_compile_${w}=${ct}" >>"$out_dir/compile_times.txt"
  done
fi

# Rust compile timing (optional cold) — one release already built; skip clean for multi-lang speed

echo "" >>"$report"
echo "## Compile time (seconds, one sample where applicable)" >>"$report"
echo "" >>"$report"
echo "See \`results/compile_times.txt\`. Interpreted langs have no separate AOT step." >>"$report"
echo "" >>"$report"
echo '```' >>"$report"
cat "$out_dir/compile_times.txt" >>"$report" 2>/dev/null || true
echo '```' >>"$report"

run_lang() {
  lang=$1
  w=$2
  out="$out_dir/${w}_${lang}.out"
  case $lang in
    ori)        "$root/bin/ori_${w}" ;;
    python)     "$PYTHON" "$root/python/${w}.py" ;;
    rust)       "$root/rust_${w}/target/release/rust_${w}" ;;
    c)          "$root/bin/c/${w}" ;;
    go)         "$root/bin/go/${w}" ;;
    javascript) "$NODE" "$root/javascript/${w}.js" ;;
    typescript) "$NODE" "$root/bin/ts/${w}.js" ;;
    ruby)       "$RUBY" "$root/ruby/${w}.rb" ;;
    nim)        "$root/bin/nim/${w}" ;;
    *) echo "unknown lang $lang" >&2; return 1 ;;
  esac >"$out"
}

time_lang() {
  lang=$1
  w=$2
  out="$out_dir/${w}_${lang}.out"
  case $lang in
    ori)        time_cmd "$out" "$root/bin/ori_${w}" ;;
    python)     time_cmd "$out" "$PYTHON" "$root/python/${w}.py" ;;
    rust)       time_cmd "$out" "$root/rust_${w}/target/release/rust_${w}" ;;
    c)          time_cmd "$out" "$root/bin/c/${w}" ;;
    go)         time_cmd "$out" "$root/bin/go/${w}" ;;
    javascript) time_cmd "$out" "$NODE" "$root/javascript/${w}.js" ;;
    typescript) time_cmd "$out" "$NODE" "$root/bin/ts/${w}.js" ;;
    ruby)       time_cmd "$out" "$RUBY" "$root/ruby/${w}.rb" ;;
    nim)        time_cmd "$out" "$root/bin/nim/${w}" ;;
  esac
}

# Header row for runtime table
echo "" >>"$report"
echo "## Runtime wall time (seconds, median of $samples)" >>"$report"
echo "" >>"$report"
echo "Pure process execution after compile. Times include process start + one-line stdout." >>"$report"
echo "" >>"$report"

# Build markdown header from active langs
hdr="| Workload"
sep="|----------"
for lang in $active_langs; do
  hdr="$hdr | $lang"
  sep="$sep |------:"
done
hdr="$hdr |"
sep="$sep |"
echo "$hdr" >>"$report"
echo "$sep" >>"$report"

# Also a ratios table relative to Ori when present
echo "" >"$out_dir/ratios_body.md"

echo "== run benchmarks (samples=$samples) langs=$active_langs =="
for w in $workloads; do
  echo "-- $w --"
  ref_out=""
  row="| \`$w\`"
  for lang in $active_langs; do
    : >"$out_dir/${w}_${lang}.times"
    # warmup
    run_lang "$lang" "$w" >/dev/null 2>&1 || true
    i=1
    while [ "$i" -le "$samples" ]; do
      time_lang "$lang" "$w" >>"$out_dir/${w}_${lang}.times"
      i=$((i + 1))
    done
    med=$(median <"$out_dir/${w}_${lang}.times")
    eval "med_${lang}=\$med"
    row="$row | ${med}"
    o=$(tr -d '\r\n' <"$out_dir/${w}_${lang}.out")
    if [ -z "$ref_out" ]; then
      ref_out=$o
      ref_lang=$lang
    elif [ "$o" != "$ref_out" ]; then
      echo "WARN result mismatch $w $lang=$o (ref $ref_lang=$ref_out)" >&2
    fi
    printf "  %-12s med=%s  out=%s\n" "$lang" "$med" "$o"
  done
  row="$row |"
  echo "$row" >>"$report"

  # Ori ratios if ori present
  if echo "$active_langs" | grep -qw ori; then
    om=$(median <"$out_dir/${w}_ori.times")
    rline="| \`$w\`"
    for lang in $active_langs; do
      if [ "$lang" = ori ]; then
        rline="$rline | 1.00×"
      else
        lm=$(median <"$out_dir/${w}_${lang}.times")
        ratio=$("$PYTHON" -c "a=float('$lm'); b=float('$om'); print(f'{a/b:.3f}×' if b>0 else 'n/a')")
        rline="$rline | $ratio"
      fi
    done
    rline="$rline |"
    echo "$rline" >>"$out_dir/ratios_body.md"
  fi
done

if [ -s "$out_dir/ratios_body.md" ]; then
  echo "" >>"$report"
  echo "## Relative to Ori (lang / Ori; lower is faster)" >>"$report"
  echo "" >>"$report"
  echo "$hdr" >>"$report"
  echo "$sep" >>"$report"
  cat "$out_dir/ratios_body.md" >>"$report"
fi

echo "" >>"$report"
echo "## Notes / fairness" >>"$report"
echo "" >>"$report"
echo "- Same algorithm shape (while/for loops, explicit indices) across languages." >>"$report"
echo "- Ori: AOT via \`ori compile\` (not JIT \`ori run\`)." >>"$report"
echo "- Rust: \`cargo build --release\`; \`black_box\` on final value. \`sum_loop\` may be strength-reduced." >>"$report"
echo "- C: \`gcc -O2 -std=c11\`." >>"$report"
echo "- Go: \`go build\` (default optimisations)." >>"$report"
echo "- Nim: \`nim c -d:release\`." >>"$report"
echo "- Python / Ruby: CPython / CRuby interpreters; fib uses 64-bit mask." >>"$report"
echo "- JavaScript / TypeScript: Node.js; TS compiled with \`tsc\` then run on Node." >>"$report"
echo "- Times include process start; not a full language ranking." >>"$report"
echo "" >>"$report"
echo "## Raw times" >>"$report"
echo "" >>"$report"
echo "See \`tools/bench/polyglot/results/*_*.times\`." >>"$report"

echo ""
echo "Report written: $report"
cat "$report"
# Refresh LATEST.md pointer content
cp "$report" "$out_dir/LATEST.md"
echo "Also copied to $out_dir/LATEST.md"
