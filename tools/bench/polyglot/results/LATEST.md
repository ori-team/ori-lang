# Polyglot bench report

- **When:** 2026-07-13T23:21:01-03:00
- **Host:** Linux 6.14.0-33-generic x86_64
- **CPU:** Intel(R) Core(TM) i7-3632QM CPU @ 2.20GHz
- **samples per cell:** 5
- **timer:** Python `time.perf_counter` (subprocess wall time)

## Toolchain versions

| Lang | Version |
|------|---------|
| ori | ori 0.3.4 |
| python | Python 3.12.3 |
| rust | rustc 1.95.0 (59807616e 2026-04-14) |
| c | gcc (Ubuntu 13.3.0-6ubuntu2~24.04) 13.3.0 |
| go | go version go1.22.2 linux/amd64 |
| javascript | node v24.18.0 |
| typescript | Version 7.0.2 / node v24.18.0 |
| ruby | ruby 3.2.3 (2024-01-18 revision 52bb2ac0a6) [x86_64-linux-gnu] |
| nim | Nim Compiler Version 1.6.14 [Linux: amd64] |

## Workloads

| Workload | Description |
|----------|-------------|
| sum_loop | sum 0..10_000_000-1 |
| fib_iter | iterative fib **20_000_000 steps** (i64 wrap where needed) |
| list_sum | push 1_000_000 ints + sum |
| nested | nested loops 2000×2000 |


## Compile time (seconds, one sample where applicable)

See `results/compile_times.txt`. Interpreted langs have no separate AOT step.

```
ori_compile_sum_loop=0.594
ori_compile_fib_iter=0.652
ori_compile_list_sum=0.765
ori_compile_nested=0.803
c_compile_sum_loop=0.302
c_compile_fib_iter=0.105
c_compile_list_sum=0.165
c_compile_nested=0.137
go_compile_sum_loop=0.765
go_compile_fib_iter=0.358
go_compile_list_sum=0.117
go_compile_nested=0.092
ts_compile_all=0.984
nim_compile_sum_loop=0.922
nim_compile_fib_iter=0.605
nim_compile_list_sum=0.613
nim_compile_nested=0.681
```

## Runtime wall time (seconds, median of 5)

Pure process execution after compile. Times include process start + one-line stdout.

| Workload | ori | python | rust | c | go | javascript | typescript | ruby | nim |
|---------- |------: |------: |------: |------: |------: |------: |------: |------: |------: |
| `sum_loop` | 0.002157 | 2.934094 | 0.001576 | 0.001323 | 0.008916 | 0.080839 | 0.076716 | 0.410046 | 0.007124 |
| `fib_iter` | 0.016023 | 7.052647 | 0.010956 | 0.014666 | 0.019805 | 1.173559 | 1.220212 | 5.992260 | 0.023948 |
| `list_sum` | 0.016190 | 0.525780 | 0.008919 | 0.010314 | 0.009832 | 0.094658 | 0.093489 | 0.197990 | 0.032323 |
| `nested` | 0.001755 | 0.968458 | 0.002202 | 0.001833 | 0.004229 | 0.060560 | 0.059814 | 0.212001 | 0.001919 |

## Relative to Ori (lang / Ori; lower is faster)

| Workload | ori | python | rust | c | go | javascript | typescript | ruby | nim |
|---------- |------: |------: |------: |------: |------: |------: |------: |------: |------: |

| `sum_loop` | 1.00× | 1360.266× | 0.731× | 0.613× | 4.134× | 37.478× | 35.566× | 190.100× | 3.303× |
| `fib_iter` | 1.00× | 440.158× | 0.684× | 0.915× | 1.236× | 73.242× | 76.154× | 373.979× | 1.495× |
| `list_sum` | 1.00× | 32.476× | 0.551× | 0.637× | 0.607× | 5.847× | 5.774× | 12.229× | 1.996× |
| `nested` | 1.00× | 551.828× | 1.255× | 1.044× | 2.410× | 34.507× | 34.082× | 120.798× | 1.093× |

## Notes / fairness

- Same algorithm shape (while/for loops, explicit indices) across languages.
- Ori: AOT via `ori compile` (not JIT `ori run`).
- Rust: `cargo build --release`; `black_box` on final value. `sum_loop` may be strength-reduced.
- C: `gcc -O2 -std=c11`.
- Go: `go build` (default optimisations).
- Nim: `nim c -d:release`.
- Python / Ruby: CPython / CRuby interpreters; fib uses 64-bit mask.
- JavaScript / TypeScript: Node.js; TS compiled with `tsc` then run on Node.
- Times include process start; not a full language ranking.

## Raw times

See `tools/bench/polyglot/results/*_*.times`.
