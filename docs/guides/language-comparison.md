# Language comparison notes

> **Status:** historical benchmark notes; not a language ranking
> **Portuguese:** [language-comparison.pt-BR.md](language-comparison.pt-BR.md)

The maintained performance comparison is [performance.md](performance.md),
backed by `tools/bench/polyglot/`. The older PowerShell suite is preserved as a
historical record in [language-comparison.pt-BR.md](language-comparison.pt-BR.md)
and must not be used as a current performance claim.

When comparing a compiler change, use the same workload, compiler flags,
runtime version, and iteration count. Report startup cost separately from the
steady-state loop. Security, diagnostics, package ergonomics, and memory
safety are not captured by a runtime-only benchmark.
