# Quality documentation

Quality documentation defines how Ori proves correctness, compatibility, safety, performance, and documentation accuracy.

## Canonical documents

- [`test-strategy.md`](test-strategy.md) — test layers, gates, and evidence.
- [`language-conformance.md`](language-conformance.md) — specification-to-test traceability.
- [`diagnostic-design.md`](diagnostic-design.md) — public diagnostic design and compatibility.
- [`performance-policy.md`](performance-policy.md) — benchmark and regression policy.
- [`../planning/qa/test-matrix-ori.md`](../planning/qa/test-matrix-ori.md) — detailed current coverage matrix.

## Quality principle

A green test suite is necessary but not sufficient. Ori quality also requires:

- the tests cover the public contract rather than only internal functions;
- AOT, JIT, tooling, runtime, and documentation agree;
- failures are diagnosable;
- performance claims are reproducible;
- security boundaries are reviewed;
- active documents match the current 0.3.8 implementation;
- regressions are prevented at the smallest useful layer.

Quality gates must not be weakened merely to make a change pass.