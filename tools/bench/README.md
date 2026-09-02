# Runtime benchmarks

These workloads are measurement tools, not release gates. Run them with a
current native `ori` binary from the repository root.

## Timer heap

`run_timer_heap_churn.sh` starts 128 concurrent ten-millisecond sleeps, joins
every task, and checks the `128` completion canary. The burst crosses the timer
heap compaction threshold and exercises cancellation-token/vector cleanup
without relying on wall-clock output for correctness.

```sh
SAMPLES=3 ./tools/bench/run_timer_heap_churn.sh
```

Set `ORI_BIN` to measure another build. The reported elapsed time is a local
baseline only; compare runs on the same host and toolchain.
