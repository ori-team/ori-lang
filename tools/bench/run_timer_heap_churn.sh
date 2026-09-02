#!/usr/bin/env sh
# Reproducible native timer-heap workload (AUD-RT-5).
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$script_dir/../.." && pwd)
ori_bin=${ORI_BIN:-$repo/compiler/target/debug/ori}
samples=${SAMPLES:-3}

if [ ! -x "$ori_bin" ]; then
  echo "timer_heap_churn: ori binary not found: $ori_bin" >&2
  exit 2
fi
case "$samples" in
  ''|*[!0-9]*|0) echo "SAMPLES must be a positive integer" >&2; exit 2 ;;
esac

bench="$script_dir/timer_heap_churn.orl"
for sample in $(seq 1 "$samples"); do
  start=$(date +%s%N)
  output=$("$ori_bin" run "$bench")
  end=$(date +%s%N)
  elapsed_ms=$(( (end - start) / 1000000 ))
  if [ "$output" != "128" ]; then
    echo "timer_heap_churn: wrong completion count: $output" >&2
    exit 1
  fi
  echo "timer_heap_churn: sample=$sample elapsed_ms=$elapsed_ms completed=128"
done
