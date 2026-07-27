# Quality documentation

Quality documentation defines how Ori proves correctness, compatibility, safety, performance, and documentation accuracy.

## Canonical documents

- [`test-strategy.md`](test-strategy.md) — test layers, gates, and evidence.
- [`language-conformance.md`](language-conformance.md) — specification-to-test traceability.
- [`diagnostic-design.md`](diagnostic-design.md) — public diagnostic design and compatibility.
- [`performance-policy.md`](performance-policy.md) — benchmark and regression policy.
- [`fuzzing.md`](fuzzing.md) — fuzz targets, corpus, triage, and CI strategy.
- [`differential-testing.md`](differential-testing.md) — AOT/JIT, optimized/reference, and backend comparisons.
- [`property-testing.md`](property-testing.md) — generated structured inputs and invariant checks.
- [`documentation-quality.md`](documentation-quality.md) — canonical ownership, correctness, accessibility, metadata, links, examples, and CI.
- [`../planning/qa/test-matrix-ori.md`](../planning/qa/test-matrix-ori.md) — detailed transitional coverage matrix.

## Quality principle

A green test suite is necessary but not sufficient. Ori quality also requires:

- tests cover the public contract rather than only internal functions;
- AOT, JIT, tooling, runtime, and documentation agree;
- failures are diagnosable;
- performance claims are reproducible;
- security boundaries are reviewed;
- active documents match the current 0.3.8 implementation;
- regressions are prevented at the smallest useful layer;
- generated/property/fuzz evidence protects classes of failures beyond hand-written examples.

Quality gates must not be weakened merely to make a change pass.