# ADR — backend after LANG-PERF-2

> **Status:** accepted
> **Date:** 2026-08-25
> **Decision:** keep Cranelift as Ori's AOT and JIT backend; shelve LLVM/ORC

## Context

`LANG-PERF-2` required an explicit backend decision after the HIR mid-end and
the first loop optimizations. The decision depended on the measured primary
kernels, not on theoretical optimizer breadth.

The versioned baseline records:

- `fib_iter` at about 1.6–2× Rust after the cycle-collection placement fix and
  mid-end work, comfortably inside the original Ori/Go goal of 10×;
- scalar `list_sum` at about 1.25× Rust, inside the 2× non-regression goal;
- `nested` at about 1.2 ms after conservative strength reduction, faster than
  the recorded 4 ms Go reference on that host.

The minimum LANG-PERF-2 implementation also exists in-tree: CLIF dumping and
the smoke harness, bounded HIR optimization pipeline, constant folding/DCE,
loop strength reduction, and conservative aggressive leaf inlining.

## Decision

Ori remains Cranelift-only for product AOT and JIT. LLVM AOT, LLVM ORC, and a
dual-backend hybrid are shelved because the acceptance kernels no longer show
a backend-sized performance gap, while a second backend would add a permanent
parity, CI, ABI, debugging, and maintenance tax.

This is not a claim that every workload is optimal. Remaining work should be
driven by real application profiles and implemented in the backend-agnostic
HIR mid-end or focused native lowering when possible.

## Reopen conditions

Reconsider an alternate backend only when a reproducible workload:

1. remains materially slower after HIR and native-lowering investigation;
2. demonstrates that the missing optimization is backend-specific rather than
   a source/HIR shape problem;
3. includes AOT/JIT parity, compile-time, binary-size, debugging, and CI cost
   in the comparison; and
4. justifies maintaining two backend conformance matrices.

## Consequences

- Cranelift remains the single native correctness matrix.
- HIR optimizations stay backend-agnostic and conservatively bounded.
- LLVM/ORC is no longer an implicit residual of `LANG-PERF-2`.
- A future proposal must bring new measurements and is a new decision, not a
  continuation of the closed performance wave.
