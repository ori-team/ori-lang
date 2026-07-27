# Performance microbench (polyglot)

> **Audience:** users and contributors who want an honest snapshot of Ori
> runtime cost on small kernels.  
> **Not** a full language ranking.  
> **Portuguese:** [performance.pt-BR.md](performance.pt-BR.md)  
> **Harness:** [`tools/bench/polyglot/`](../../tools/bench/polyglot/)  
> **Latest machine report:** [`tools/bench/polyglot/results/LATEST.md`](../../tools/bench/polyglot/results/LATEST.md)

## Snapshot (2026-07-14, loop-GC fix + mid-end)

| Item | Value |
|------|--------|
| Host | Linux x86_64 · Intel Core i7-3632QM @ 2.20 GHz |
| Samples | **5** (median wall time) |
| Timer | `time.perf_counter` around the process (µs) |
| Ori | **0.3.4** AOT (`ori compile`, mid-end **Default**) |
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
