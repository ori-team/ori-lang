#!/usr/bin/env sh
# Graphics microbench suite: Ori AOT software-rendering kernels.
# GFX-BENCH-01..06 — see README.md for the workload matrix.
#
# Usage (from repo root):
#   SAMPLES=3 ./tools/bench/graphics/run_graphics_bench.sh
#
# Metrics recorded per kernel: wall time (Python perf_counter), derived
# pixels/s or vertices/s, compile time, and AOT-vs-JIT comparison when the
# `ori` binary is the dev build (JIT is `ori run`, AOT is `ori compile`).
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

ORI_BIN="${ORI_BIN:-$(command -v ori || true)}"
PYTHON="${PYTHON:-python3}"
samples="${SAMPLES:-5}"
out_dir="$root/results"
mkdir -p "$out_dir" "$root/bin"
stamp=$(date -Iseconds 2>/dev/null || date)
report="$out_dir/report_$(date +%Y%m%d_%H%M%S).md"
: >"$out_dir/compile_times.txt"
: >"$out_dir/run_times.txt"

workloads="gfx01_fill gfx02_gradient gfx03_line gfx04_triangle gfx05_zbuffer gfx06_vertex"

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

if [ -z "$ORI_BIN" ] || [ ! -x "$ORI_BIN" ]; then
  echo "SKIP: no \`ori\` binary found. Set ORI_BIN or build with:" >&2
  echo "  cargo build -p ori-driver && ORI_BIN=compiler/target/debug/ori $0" >&2
  exit 1
fi

echo "# Graphics bench report" >"$report"
echo "" >>"$report"
echo "- **When:** $stamp" >>"$report"
echo "- **Host:** $(uname -srm)" >>"$report"
echo "- **CPU:** $(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | sed 's/^ //' || echo unknown)" >>"$report"
echo "- **samples per cell:** $samples" >>"$report"
echo "- **timer:** Python \`time.perf_counter\` (subprocess wall time)" >>"$report"
echo "- **ori:** $($ORI_BIN --version 2>/dev/null || echo unknown)" >>"$report"
echo "" >>"$report"
echo "## Kernels" >>"$report"
echo "" >>"$report"
echo "| Kernel | Work | Metric |" >>"$report"
echo "|--------|------|--------|" >>"$report"
echo "| gfx01_fill | fill 640x480 RGBA | pixels/s |" >>"$report"
echo "| gfx02_gradient | x/y gradient 640x480 | pixels/s |" >>"$report"
echo "| gfx03_line | 512 Bresenham lines | lines/s |" >>"$report"
echo "| gfx04_triangle | 256 edge-function triangles | triangles/s |" >>"$report"
echo "| gfx05_zbuffer | 256 z-buffered triangles | triangles/s |" >>"$report"
echo "| gfx06_vertex | 2k Vec4 x Mat4 | vertices/s |" >>"$report"
echo "" >>"$report"

# ── Compile AOT ───────────────────────────────────────────────────────────────
echo "Compiling kernels (AOT)…"
for w in $workloads; do
  src="$root/ori/${w}.orl"
  bin="$root/bin/${w}"
  tmpbin="/tmp/ori_gfx_${w}_$$"
  t0=$("$PYTHON" -c 'import time; print(time.perf_counter())')
  "$ORI_BIN" compile "$src" --out "$tmpbin" >/dev/null 2>&1
  mv -f "$tmpbin" "$bin"
  chmod +x "$bin"
  t1=$("$PYTHON" -c 'import time; print(time.perf_counter())')
  ct=$("$PYTHON" -c "print(f'{$t1 - $t0:.3f}')")
  echo "ori_compile_${w}=${ct}" >>"$out_dir/compile_times.txt"
  echo "  ${w}: compiled in ${ct}s"
done

# ── Run AOT (multiple samples, keep per-sample times) ────────────────────────
echo "Running AOT kernels…"
for w in $workloads; do
  : >"$out_dir/${w}.times"
  for s in $(seq 1 "$samples"); do
    t=$(time_cmd "$out_dir/${w}.out" "$root/bin/${w}")
    echo "$t" >>"$out_dir/${w}.times"
  done
  med=$(median <"$out_dir/${w}.times")
  echo "${w}=${med}" >>"$out_dir/run_times.txt"
  echo "  ${w}: median ${med}s"
done

# ── Derived metrics ───────────────────────────────────────────────────────────
echo "" >>"$report"
echo "## AOT results (median of $samples runs)" >>"$report"
echo "" >>"$report"
echo "| Kernel | Time (s) | Throughput |" >>"$report"
echo "|--------|----------|------------|" >>"$report"

pixels_per_s() {
  # 640*480 = 307200
  "$PYTHON" -c "print(f'{307200 / float($1):.0f}')"
}
lines_per_s() {
  "$PYTHON" -c "print(f'{512 / float($1):.0f}')"
}
tris_per_s() {
  "$PYTHON" -c "print(f'{256 / float($1):.0f}')"
}
verts_per_s() {
  "$PYTHON" -c "print(f'{2000 / float($1):.0f}')"
}

for w in $workloads; do
  med=$(median <"$out_dir/${w}.times")
  case "$w" in
    gfx01_fill|gfx02_gradient) tp="$(pixels_per_s "$med") px/s" ;;
    gfx03_line) tp="$(lines_per_s "$med") lines/s" ;;
    gfx04_triangle|gfx05_zbuffer) tp="$(tris_per_s "$med") tris/s" ;;
    gfx06_vertex) tp="$(verts_per_s "$med") verts/s" ;;
  esac
  echo "| $w | $med | $tp |" >>"$report"
done

echo "" >>"$report"
echo "## Compile times (AOT)" >>"$report"
echo "" >>"$report"
echo '```text' >>"$report"
cat "$out_dir/compile_times.txt" >>"$report"
echo '```' >>"$report"

echo "" >>"$report"
echo "## Fairness notes" >>"$report"
echo "" >>"$report"
echo "- Process wall time (startup + work + one print)." >>"$report"
echo "- Framebuffers use \`list[int]\` until GFX-BUFFER-1 lands (contiguous \`buffer[T]\`)." >>"$report"
echo "- Depth buffer uses fixed-point ints (software-renderer standard)." >>"$report"
echo "- No bitwise ops yet (GFX-BITWISE-1 pending); packing uses multiply-add." >>"$report"

echo ""
echo "Report: $report"
echo "All kernels: $out_dir/run_times.txt"
