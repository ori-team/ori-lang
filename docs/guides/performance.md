# Performance microbench (polyglot)

> **Audience:** users and contributors who want an honest snapshot of Ori
> runtime cost on small kernels.  
> **Not** a full language ranking.  
> **Portuguese:** [performance.pt-BR.md](performance.pt-BR.md)  
> **Harness:** [`tools/bench/polyglot/`](../../tools/bench/polyglot/)  
> **Latest machine report:** [`tools/bench/polyglot/results/LATEST.md`](../../tools/bench/polyglot/results/LATEST.md)

This page preserves the **2026-07-14 historical measurement**. It is not a
claim about the current `0.3.8-dev` workspace; rerun the harness before using
the numbers for a current decision.

## Snapshot (2026-07-14, loop-GC fix + mid-end)

| Item | Value |
|------|--------|
| Host | Linux x86_64 · Intel Core i7-3632QM @ 2.20 GHz |
| Samples | **5** (median wall time) |
| Timer | `time.perf_counter` around the process (µs) |
| Ori | **0.3.4** AOT (`ori compile`, mid-end **Default**, historical) |
| Python | CPython **3.12.3** |
| Rust | **1.95.0** release |
| C | **gcc 13.3** `-O2` |
| Go | **1.22.2** |
| JavaScript | **Node v24.18** |
| TypeScript | **tsc 7.0** → Node |
| Ruby | **3.2.3** (CRuby) |
| Nim | **1.6.14** `-d:release` |

Same algorithm shape (`while` / explicit indices). Printed results match across
all languages on every kernel.

**What landed for this snapshot:**

1. Native `while`/`for` no longer call `ori_arc_collect_cycles` every iteration.
2. HIR mid-end Default: const fold + pure-loop **strength reduction** + DCE.
3. `ORI_OPT=aggressive` adds monomorphic leaf inlining (little effect on these
   single-function kernels).

### Runtime (median seconds)

| Workload | Ori | Python | Rust | C | Go | JS | TS | Ruby | Nim |
|----------|-----|--------|------|---|-----|----|----|------|-----|
| `sum_loop` Σ 0..10⁷ | **0.0022**\* | 2.93 | 0.0016\* | 0.0013\* | 0.0089 | 0.081 | 0.077 | 0.410 | 0.0071 |
| `fib_iter` 2·10⁷ steps | **0.016** | 7.05 | 0.011 | 0.015 | 0.020 | 1.17 | 1.22 | 5.99 | 0.024 |
| `list_sum` 10⁶ push+sum | **0.011**† | 0.53 | 0.0089 | 0.010 | 0.0098 | 0.095 | 0.093 | 0.198 | 0.032 |
| `nested` 2000×2000 | **0.0018**\* | 0.97 | 0.0022 | 0.0018 | 0.0042 | 0.061 | 0.060 | 0.212 | 0.0019 |

\* Pure sum/nested often become closed forms (Ori mid-end; Rust/C optimisers).
Prefer **`fib_iter`** and **`list_sum`** for loop / heap cost.  
† After scalar list inline codegen + `with_capacity` (2026-07-14 remeasure vs
Rust ~0.009 s on the same host ≈ **1.25×**). Other columns still from the full
polyglot suite unless noted.

### Extended suite — September 2026 (ori 0.3.8, expanded 8-workload polyglot)

Measured on Intel i7-3632QM up-to-date September 2026 run (median of 3 samples, AOT):

| Workload | Ori | Python | Rust | C | Go | JS | Ruby |
|----------|-----|--------|------|---|-----|----|------|
| `vec4_simd` 5M 4D vector additions | **0.009** | 1.527 | 0.007 | 0.008 | 0.010 | 0.081 | 1.868 |
| `arena_bulk_alloc` 100k frame resets | **0.015** | 0.032 | 0.002 | 0.002 | 0.002 | 0.044 | 0.090 |
| `channel_throughput` 100k msgs | **0.125** | 0.049 | 0.006 | 0.001 | 0.009 | 0.062 | 0.104 |
| `spatial_grid_bvh` 1M AABB tests | **0.219** | 0.309 | 0.002 | 0.001 | 0.004 | 0.057 | 0.490 |

**Key takeaways:**

1. **SIMD vectorization (`vec4_simd`)**: Ori lowers `simd[float32, 4]` directly to Cranelift `F32X4` registers,
   completing 5M vector additions in **8.5 ms** (≈1.1× GCC -O2, ≈1.2× Rust), surpassing Go (10.3 ms),
   Node (80 ms), and beating Python by **~178×**.
2. **Bulk arena reset (`arena_bulk_alloc`)**: Ori's `mem.Region` accumulator overhead (~15 ms for 100k resets)
   comes from repeated native `ori_region_reset` FFI round-trips per iteration, while Rust/C reset their buffer
   offset in-process (1–2 ms). The O(1) cost is the runtime-API call itself, not the reset mechanics.
3. **Managed channel pacing (`channel_throughput`)**: Ori's synchronized `ori_channel_send`/`receive` path uses the
   global ARC task-aware queue (~125 ms), slower than Go's dedicated goroutine runtime and Rust's `crossbeam`.
   A lock-aware rebalance or lock-free ring would tighten this 1–2 orders of magnitude.
4. **Struct passing and devirtualization (`spatial_grid_bvh`)**: Ori passes `AABB` structs by value against the full
   call ABI, while C/Rust inline the accessor entirely (1–2 ms). A compiler inline threshold or leaf-specialization
   would close most of this gap.

### Relative to Ori (lang / Ori; **lower is faster**)

| Workload | Py | Rust | C | Go | JS | TS | Ruby | Nim |
|----------|-----|------|---|-----|----|----|------|-----|
| `sum_loop` | **1360×** | 0.73×\* | 0.61×\* | 4.1× | 37× | 36× | 190× | 3.3× |
| `fib_iter` | **440×** | **0.68×** | 0.92× | 1.24× | 73× | 76× | 374× | 1.50× |
| `list_sum` | **48×**† | **0.78×**† | ~0.9× | ~0.9× | ~9× | ~8× | ~18× | ~2.9× |
| `nested` | **552×** | **1.26×** | 1.04× | 2.4× | 35× | 34× | 121× | 1.09× |

## How to read this

### Ori vs interpreters

| Peer | Takeaway |
|------|----------|
| **Python** | Ori is about **30–1400×** faster on these kernels |
| **Ruby** | Ori is about **12–370×** faster |
| **JS / TS (Node)** | Ori wins all four (**~6–75×**) |

### Ori vs AOT / systems languages

| Peer | Takeaway |
|------|----------|
| **`fib_iter`** | Best non-closed-form signal: Ori **~1.5×** Rust, **beats Go and Nim**, near C |
| **`list_sum`** | Ori **~1.25×** Rust with `with_capacity` + **inline scalar push/get** (was ~1.8×); gap is checks/version, not ARC on `list[int]` |
| **`sum` / `nested`** | Closed-form noise floor; Ori competitive with C/Rust when reduced |
| **Go / Nim** | No longer dominate Ori on fib after the loop GC fix |

### Positioning (pre-1.0)

- Clearly **ahead of CPython, CRuby, and Node**.
- **Competitive with mature AOT** on tight fib (within ~1.5× of Rust).
- Remaining gap is mostly **list/ARC** and further mid-end/codegen polish
  (`ORI_OPT=aggressive` leaf inline for real multi-function code).

### Mid-end flags

| `ORI_OPT` | Passes |
|-----------|--------|
| `none` / `0` | No HIR rewrites |
| `default` (unset) | Const fold + pure-loop strength reduction + DCE |
| `aggressive` / `2` | Default + monomorphic leaf inlining |

## Fairness / caveats

1. Same source shape across languages.
2. Ori path is **AOT** (`ori compile`), not JIT `ori run`.
3. Python / Ruby fib use a **64-bit mask** so bigints do not dominate.
4. JS/TS fib use BigInt with 64-bit wrap for parity with i64.
5. Nim uses `{.push overflowChecks: off.}` for wrapping i64 fib.
6. Rust/C/Ori may strength-reduce simple reductions (`sum_loop` / pure nested).
7. Times include process start and one-line stdout.
8. Host is a laptop CPU; **ratios matter more than absolute milliseconds**.
9. This does **not** measure I/O, async, FFI, multi-file projects, or real apps.

## Zero-allocation hot paths (`@noalloc` + `mem.region` + `simd`)

The high-performance systems wave (2026-09-03) provides general-purpose zero-allocation
primitives for 60/120 FPS frame loops:

- `@noalloc` statically checks that marked functions perform no dynamic heap allocations
  (rejecting `list`/`map`/`set`, string formatting, closures, `await`, `using`, and allocating calls).
- `using r: mem.Region = mem.region()` creates bump-arenas with O(1) bulk resets (`mem.reset`),
  eliminating per-object reference-counting overhead in tick loops.
- `simd[float32, 4]` lowers directly to CPU vector registers (x86_64 SSE/AVX and ARM NEON)
  with parallel vector arithmetic (`+`, `-`, `*`, `/`).
- `@align(N)` explicitly aligns struct layouts for GPU uniform buffers (`alignas(N)`).

## How to reproduce

Requires `ori`, `python3`, `cargo`/`rustc`, `gcc`, `go`, `node`, `tsc`, `ruby`, `nim`
on `PATH` (missing langs are skipped).

```bash
SAMPLES=5 ./tools/bench/polyglot/run_polyglot_bench.sh
# SAMPLES=3 is fine for a quick smoke
# ORI_OPT=none ./tools/bench/polyglot/run_polyglot_bench.sh  # mid-end off
```

Sources: `tools/bench/polyglot/{ori,python,rust_*,c,go,javascript,typescript,ruby,nim}/`.

## Related docs

| Document | Role |
|----------|------|
| [tools/bench/polyglot/README.md](../../tools/bench/polyglot/README.md) | Harness layout |
| [results/LATEST.md](../../tools/bench/polyglot/results/LATEST.md) | Full machine report |
| [language-comparison.md](language-comparison.md) | Older PowerShell multi-lang suite (historical) |
| [../planning/perf-baseline-2026-07-13.md](../planning/perf-baseline-2026-07-13.md) | Compiler-side LANG-PERF baseline |
| [../planning/historico/perf-runtime-midend-plan.md](../planning/historico/perf-runtime-midend-plan.md) | LANG-PERF-2 mid-end plan |
