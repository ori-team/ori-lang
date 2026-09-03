# Performance baseline (LANG-PERF)

> Host: Linux x86_64 · Date: 2026-07-13  
> Binary: `compiler/target/release/ori`  
> Runtime: staged under `runtime/<triple>/` (includes `runtime/bin/rust-lld`)

## Debug (pre-work, for comparison)

| Workload | Mode | Wall time |
|----------|------|-----------|
| `examples/hello` | check | ~0.02 s |
| `examples/hello` | run (JIT) | ~0.50 s |
| `examples/hello` | compile AOT | (not measured) |

## Release — before LANG-PERF changes

| Workload | Mode | Wall time (≈ 3 samples) |
|----------|------|-------------------------|
| `examples/hello` | check | ~0.00–0.00 s |
| `examples/hello` | run (JIT) | ~0.13–0.16 s |
| `examples/calculator` | run (JIT) | ~0.12–0.31 s |
| `examples/language_features` | run (JIT) | ~0.17–0.19 s |
| `examples/collections_demo` | run (JIT) | ~0.14–0.18 s |
| `examples/hello` | compile AOT (system `ld`) | ~4.0 s first / ~2.5 s second |
| AOT binary only | execute | ~0.00 s |

Default linker then: **SystemLinker** (`ld`).

## Release — after LANG-PERF (this wave)

### Changes

1. **Cranelift product flags:** `enable_verifier=false`; AOT `opt_level=speed`;
   JIT `opt_level=none` (fast lower for `ori run`).
2. **Default linker order:** discover **BundledRustLld** first when available
   (packaged `runtime/bin/rust-lld`), then SystemLinker, then rustc driver.
   Still no `rustc` for end users with package layout.

### Numbers

| Workload | Mode | Wall time (≈ samples) |
|----------|------|------------------------|
| `examples/hello` | check | ~0.00–0.02 s |
| `examples/hello` | run (JIT) | ~0.11–0.27 s (median ≈ 0.15) |
| `examples/calculator` | run (JIT) | ~0.10–0.20 s |
| `examples/language_features` | run (JIT) | ~0.12–0.23 s |
| `examples/collections_demo` | run (JIT) | ~0.09–0.15 s |
| `examples/hello` | compile AOT | **~0.93–1.03 s** |
| AOT binary only | execute | ~0.00 s |

`ori doctor` reports: `linker strategy — BundledRustLld (default)`.

### Wins

| Path | Before | After | Notes |
|------|--------|-------|-------|
| AOT `ori compile` hello | ~2.5–4 s | **~1.0 s** | ~2.5–4× faster with default packaged lld + CL flags |
| JIT `ori run` tiny | ~0.13–0.16 s | ~0.11–0.15 s | modest; dominated by Cranelift lower + cdylib |
| `ori check` tiny | already sub-20 ms | unchanged | |

### Residual (wave 1 — closed in wave 2 where noted)

- JIT cold start still ~100+ ms for hello (codegen + load runtime) — **open**.
- Multi-file / ARC-heavy program microbench suite — **added** `tools/microbench_lang_perf.sh`.
- Optional: mold/ld.lld discovery if present on PATH — **done** (SystemLinker Linux).
- Release-runtime stage default — **done** (`stage_native_runtime.*` default release).

## Wave 2 (2026-07-13) — linker PATH + stage release + microbench

### Changes

1. **Linux SystemLinker:** PATH order `mold` → `ld.lld` → `ld`, then
   `cc -print-prog-name=ld`. Override: `ORI_SYSTEM_LINKER`.
2. **Stage scripts** default **`--profile release`** (`ORI_STAGE_PROFILE` or
   `--profile debug` for runtime iteration).
3. **Harness:** `tools/microbench_lang_perf.sh` samples check/run/compile on
   `hello`, `multi_module`, `collections_demo`, `language_features`, `concurrency`.

### Numbers (release `ori` + release staged runtime + BundledRustLld)

Host Linux x86_64 · warm process · 2026-07-13 afternoon remeasure:

| Workload | Mode | Wall time |
|----------|------|-----------|
| `hello` | check | ~0.00 s |
| `hello` | run (JIT) | **~0.04–0.05 s** |
| `hello` | compile AOT (BundledRustLld) | **~0.19–0.20 s** |
| `hello` | compile AOT (`ORI_USE_SYSTEM_LINKER=1`, BFD `ld`) | ~0.36 s |
| `multi_module` | run / compile | ~0.05 s / ~0.19 s |
| `collections_demo` | run / compile | ~0.06 s / ~0.21 s |
| `language_features` | run / compile | ~0.07 s / ~0.22 s |
| `concurrency` | run / compile | ~0.04 s / ~0.19 s |

### Wins vs wave 1 baseline

| Path | Wave 1 | Wave 2 | Notes |
|------|--------|--------|-------|
| AOT `compile` hello | ~1.0 s | **~0.20 s** | release runtime + warm; still BundledRustLld |
| JIT `run` hello | ~0.11–0.15 s | **~0.04–0.05 s** | release cdylib dominates residual |
| SystemLinker vs BFD | n/a | ~0.36 s | PATH mold/lld preferred when installed |

### Residual after wave 2 (closed in wave 3 / living)

- JIT cold start (Cranelift lower + cdylib load) still dominates `ori run` tiny
  relative to AOT binary exec (~0 s) — **living**, not a v1 gate (~40–50 ms today).
- Deeper ARC end-to-end bench — **done** (`tools/bench/arc_list_churn.orl`).
- Prefer mold over BundledRustLld by default — **wontfix** for now: BundledRustLld
  is the measured product default and does not require mold installed.

## Wave 3 (2026-07-13) — ARC bench + LANG-PERF closed

### Changes

1. **`tools/bench/arc_list_churn.orl`** — 200 rounds × 500 `list.push` + nested list
   (ARC retain/release pressure).
2. **`tools/microbench_lang_perf.sh`** includes the ARC workload.
3. **BACKLOG:** `LANG-PERF` → **done**. Further JIT gains are Cranelift-bound
   living work, not an open implementation item.

### Numbers (release + staged release runtime)

| Workload | Mode | Wall time |
|----------|------|-----------|
| `arc_list_churn` | run (JIT) | ~0.05 s |
| `arc_list_churn` | compile AOT | ~0.20 s |
| `arc_list_churn` AOT binary | execute | ~instant (workload itself is light at N=200×500) |

### How to re-measure

```bash
cd compiler && cargo build -p ori-driver --release
../tools/stage_native_runtime.sh          # release runtime + rust-lld by default
../tools/microbench_lang_perf.sh --skip-stage
# or one-shot:
/usr/bin/time -f '%e' ./target/release/ori compile ../examples/hello --out /tmp/h
/usr/bin/time -f '%e' ./target/release/ori run ../examples/hello
/usr/bin/time -f '%e' ./target/release/ori run ../tools/bench/arc_list_churn.orl
```

## Wave 4 (2026-07-13) — polyglot multi-language

Cross-language **runtime** microbench (not compiler `check`/`run` latency):

| Item | Location |
|------|----------|
| Harness | `tools/bench/polyglot/` |
| Languages | Ori, Python, Rust, C, Go, JS, TS, Ruby, Nim |
| User guide (EN) | `docs/guides/performance.md` |
| User guide (PT) | `docs/guides/performance.pt-BR.md` |
| Machine report | `tools/bench/polyglot/results/LATEST.md` |
| README snapshot | root `README.md` § Performance snapshot |

### Headline medians (Linux x86_64, i7-3632QM, 3 samples)

| Kernel | Ori | Py | Rust | C | Go | JS | TS | Ruby | Nim |
|--------|-----|-----|------|---|-----|----|----|------|-----|
| sum 10⁷ | 0.33 | 3.21 | 0.002\* | 0.001\* | 0.017 | 0.10 | 0.09 | 0.50 | 0.007 |
| fib 2e7 | 0.65 | 11.2 | 0.009 | 0.013 | 0.023 | 1.60 | 1.60 | 7.98 | 0.019 |
| list 1e6 | **0.017** | 1.00 | 0.010 | 0.011 | 0.014 | 0.14 | 0.19 | 0.27 | 0.030 |
| nested 2k² | 0.12 | 1.04 | 0.004 | 0.002 | 0.004 | 0.08 | 0.07 | 0.21 | 0.002 |

\* Rust/C `sum_loop` may strength-reduce. Prefer fib/list for AOT comparisons.

### Reading for LANG-PERF backlog

- Ori **beats CPython and CRuby** (expected AOT vs interpreters).
- Ori **near Rust/C/Go on list** (~1.2–1.6×) — ARC/list path is not the main cliff.
- Ori **far behind** C/Rust/Go/Nim on tight integer loops — mid-end / codegen.
- Node can beat Ori on simple arithmetic; Ori wins fib + list vs Node.

```bash
SAMPLES=3 ./tools/bench/polyglot/run_polyglot_bench.sh
```

## Wave 5 (2026-07-14) — LANG-PERF-2-0/1/2 land

### Changes

1. **Cycle collector placement:** `emit_scope_cleanup_calls_from` no longer
   calls `ori_arc_collect_cycles` on every block that entered with an empty
   managed stack (including `while`/`for` bodies). Collect only when
   `managed_start == 0` **and** `loop_stack` is empty.
2. **HIR mid-end:** `ori_hir::optimize` — const fold + DCE; env `ORI_OPT`.
3. **Instrument:** `ORI_DUMP_CLIF=1` or path; `tools/qa/perf_polyglot_smoke.sh`.

### Fib_iter (20M steps, same host class)

| Binary | Median wall (≈5 samples) |
|--------|---------------------------|
| Ori **before** | ~0.50–0.65 s |
| Ori **after** | **~0.018–0.046 s** (load-dependent) |
| Rust release | ~0.009–0.029 s |

≈ **1.6–2×** Rust (was ~50×). **G1 essentially met** for fib.

## Wave 6 (2026-07-14) — LANG-PERF-2-3/4 strength + leaf inline

### Changes

1. **Strength reduction (Default):** pure `while i < n { s = s + i; i++ }` →
   closed form `s = n*(n-1)/2`; nested count → `s = n*n`.
2. **Leaf inlining (Aggressive):** same-module monomorphic `return expr` leafs
   only; `ORI_OPT=aggressive`.
3. Unit tests in `ori-hir::optimize`.

### Ori AOT (release, 5 samples, same host)

| Kernel | `ORI_OPT=none` | Default (strength) | Aggressive |
|--------|----------------|--------------------|------------|
| sum_loop 10⁷ | ~0.010 s | **~0.0014 s** | ~0.0011 s |
| fib_iter 2·10⁷ | ~0.017 s | ~0.016 s | ~0.018 s |
| list_sum 10⁶ | ~0.013 s | ~0.013 s | ~0.016 s |
| nested 2000² | ~0.004 s | **~0.0012 s** | ~0.0014 s |

Strength reduction wins on pure sum/nested; fib/list unchanged (no pattern).
Full polyglot table: `tools/bench/polyglot/results/LATEST.md`.

## Wave 7 (2026-07-14) — LANG-PERF-2-5 list reserve

### API (additive, FREEZE-safe)

| Surface | Runtime |
|---------|---------|
| `ori.list.with_capacity(n)` | `ori_list_with_capacity` |
| `ori.list.capacity(xs)` | `ori_list_capacity` |
| `ori.list.reserve(xs, n)` | `ori_list_reserve` |

Push/insert share `list_ensure_capacity`; `slice` pre-sizes output.

### list_sum 10⁶ (same host, release, 5 samples)

| Variant | Median wall |
|---------|-------------|
| Before (grow from 8, doubling) | ~0.016 s |
| After (`with_capacity(n)`) | **~0.014 s** |
| Rust `Vec::with_capacity` | ~0.009 s |

Reserve removes realloc churn; remaining gap was per-iter runtime calls.

## Wave 8 (2026-07-14) — list scalar inline codegen

### Change

Native backend inlines `list[T]` **push** (when `len < cap`) and **get**
(bounds-checked) for non-managed integer/bool element types. Grow and OOB
still call the runtime. Managed elements keep push + ARC edge registration.

### list_sum 10⁶ (same host, release, ~7 samples)

| Binary | Median wall |
|--------|-------------|
| Ori before inline (~reserve only) | ~0.014 s |
| Ori **after** inline + `with_capacity` | **~0.011 s** |
| Rust `Vec::with_capacity` | ~0.0086 s |

≈ **1.25×** Rust (was ~1.8×). Residual: capacity/bounds branches, version
bump, process start — not ARC on `list[int]`.

## Wave 9 (2026-07-23) — large-module compiler scalability

The synthetic probe generates one source file with 10,000 scalar functions and
an entry point that calls all of them. `ORI_INTERNAL_PIPELINE_TIMINGS=1`
reports each compiler stage.

### Root causes and changes

1. Type checking and HIR lowering now index function signatures by `DefId`
   instead of repeatedly scanning the complete signature list.
2. Each scalar Cranelift body imports only the user/runtime symbols it actually
   references; complex managed, trait and async bodies keep the conservative
   full set.
3. Named-function closure wrappers are declared and emitted only when a
   function is used as a first-class value.
4. The per-function selector performs indexed lookups instead of scanning the
   entire symbol table.

### Result (Linux x86_64 development host)

| Probe | Before | After |
|-------|-------:|------:|
| `ori compile`, 1,000 functions | 24.4 s | 3.2 s before the final direct-lookup fix |
| `ori compile`, 10,000 functions | ~4 min reported | **21.2 s** |
| 10k frontend + type checking | not separated | **~5.0 s** |
| 10k HIR lowering | 2.19 s before signature index | **1.20 s** |
| 10k native codegen | >60 s before final lookup fix | **12.9–14.2 s** |

The permanent ignored probe lives in
`ori-driver/tests/performance_guard.rs`. Strict defaults are 10 seconds for
`check` and 60 seconds for native `compile`.

```bash
ORI_PERF_STRICT=1 \
ORI_PERF_LARGE_FUNCTION_COUNT=10000 \
cargo test -p ori-driver --test performance_guard \
  measure_check_large_single_file_scaling -- --ignored --nocapture

ORI_PERF_STRICT=1 \
ORI_PERF_LARGE_FUNCTION_COUNT=10000 \
ORI_PERF_LARGE_COMPILE=1 \
cargo test -p ori-driver --test performance_guard \
  measure_check_large_single_file_scaling -- --ignored --nocapture
```

## Wave 10 (2026-08-25) — frontend signature indexes and iterative CT-0

Two focused debug-profile guards extend the compiler scalability baseline:

| Probe | Generated shape | Pipeline wall time |
|---|---:|---:|
| Checker signature families | 1,000 each of values, structs, enums and impls, with field/method/variant lookups | **1.366–1.369 s** |
| CT-0 dependency chain | 10,000 module constants | **0.46–0.91 s** |

The checker builds immutable lookup indexes for definition signatures, fields,
variants, trait methods and `(type, trait)` / `(type, method)` implementations.
The module-constant evaluator walks dependency chains with an explicit DFS
stack and memoization, so valid generated depth is limited by available heap
instead of the Rust call stack. A separate 512-node cycle regression preserves
`consteval.cycle` without recursive evaluation.

Reproduce the strict guards:

```bash
cd compiler
ORI_PERF_STRICT=1 cargo test -p ori-driver --test performance_guard \
  check_large_signature_families_stay_within_indexed_budget -- --ignored --nocapture

ORI_PERF_STRICT=1 cargo test -p ori-driver --test performance_guard \
  check_ten_thousand_const_dependencies_stays_within_linear_budget -- --ignored --nocapture
```

These are compiler-pipeline measurements, not release-runtime comparisons.
Absolute budgets remain opt-in because shared CI hosts vary; correctness and
bounded completion remain mandatory in the normal regression suites.
