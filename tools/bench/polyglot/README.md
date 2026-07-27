# Polyglot microbench

Equivalent tiny kernels across **Ori, Python, Rust, C, Go, JavaScript,
TypeScript, Ruby, Nim**.

**User-facing write-up:**  
[docs/guides/performance.md](../../../docs/guides/performance.md) (EN) ·
[docs/guides/performance.pt-BR.md](../../../docs/guides/performance.pt-BR.md) (PT)

**Latest results:** [results/LATEST.md](results/LATEST.md)

## Layout

```text
polyglot/
  ori/  python/  rust_*/  c/  go/  javascript/  typescript/  ruby/  nim/
  bin/                 generated binaries (gitignored)
  results/             times + reports
  run_polyglot_bench.sh
```

## Workloads

| Name | Work |
|------|------|
| `sum_loop` | sum `0 .. 10_000_000-1` |
| `fib_iter` | 20_000_000 iterative fib steps (i64 wrap) |
| `list_sum` | push 1_000_000 ints + sum |
| `nested` | 2000×2000 nested increments |

## Run

```bash
# from repo root — missing languages are skipped
SAMPLES=3 ./tools/bench/polyglot/run_polyglot_bench.sh
```

| Tool | Used for |
|------|----------|
| `ori` | Ori AOT |
| `python3` | CPython |
| `cargo` / `rustc` | Rust release |
| `gcc` or `cc` | C `-O2` |
| `go` | Go build |
| `node` | JavaScript |
| `tsc` + `node` | TypeScript → JS |
| `ruby` | CRuby |
| `nim` | Nim `-d:release` |

## Fairness

- Process wall time (startup + one print).
- Ori is AOT, not JIT `ori run`.
- Fib uses 64-bit wrap in dynamic languages (no bigint blow-up).
- Rust/C may strength-reduce `sum_loop` — prefer fib/list for AOT comparisons.
