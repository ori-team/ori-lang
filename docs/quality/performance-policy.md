# Performance policy

Performance work must be reproducible, representative, and subordinate to correctness.

## Performance dimensions

Ori tracks several independent dimensions:

- lex/parse/check latency;
- HIR lowering and optimization time;
- native object generation;
- link time;
- JIT startup and execution;
- program runtime;
- managed allocation, retain/release, and cycle collection;
- memory peak and retained memory;
- binary and runtime artifact size;
- incremental rebuild reuse;
- LSP interactive latency;
- package/install/update time.

Improving one dimension does not justify hiding a regression in another.

## Benchmark classes

### Microbenchmarks

Measure a focused primitive or compiler path. They are useful for regression detection but do not prove application performance.

### Kernel benchmarks

Measure representative algorithms such as loops, collections, string processing, recursion, async scheduling, and file processing.

### Realistic projects

Measure complete multi-module programs, including compile, link, startup, runtime, and resource behavior.

### Cross-language comparisons

May provide context, but require extra care because compilers and runtimes can eliminate different work.

A cross-language claim must state:

- exact source for every language;
- compiler/interpreter versions;
- optimization flags;
- whether the result is consumed;
- warm-up policy;
- sample count and statistic;
- hardware and OS;
- known optimizer elimination or strength reduction.

## Measurement protocol

A benchmark report should include:

- project commit;
- Ori version and ABI where relevant;
- target triple;
- debug or release mode;
- optimization level and environment flags;
- linker strategy;
- runtime artifact used;
- CPU, memory, OS, and relevant power settings;
- input data;
- warm-up count;
- measured sample count;
- median and variability;
- baseline comparison;
- raw result location.

Prefer median for short noisy runs. Include percentiles or spread for latency-sensitive tooling.

## Preventing invalid benchmarks

Benchmarks must preserve intended work.

- Consume results.
- Use inputs that are not all compile-time constants when measuring loops.
- Inspect generated behavior when results are unexpectedly tiny.
- Separate algorithm cost from I/O and process startup.
- Avoid comparing debug builds with optimized builds.
- Do not compare JIT warm-up to long-running optimized code without stating it.
- Use realistic managed values when measuring ARC.

## Regression policy

A performance regression should block merge when it is:

- statistically repeatable;
- outside expected noise;
- on a protected workload;
- not justified by correctness, safety, or a documented product trade-off.

Thresholds should be workload-specific rather than one universal percentage.

Initial guidance:

- critical hot-path regressions above 5% require investigation;
- compiler or LSP latency regressions above 10% require explanation;
- artifact-size growth above 10% requires attribution;
- new asymptotic behavior is blocking even when small fixtures hide it.

These are review triggers, not automatic proof of failure.

## Performance changes

Before optimizing:

1. reproduce the workload;
2. profile or instrument the suspected path;
3. identify algorithmic complexity and allocation behavior;
4. confirm the current bottleneck;
5. add a benchmark or guard;
6. implement the smallest justified change;
7. run semantic differential tests;
8. measure broad side effects.

Do not optimize based only on code appearance.

## Runtime-specific review

ARC and runtime changes should inspect:

- retains and releases per operation;
- allocation count and size;
- lock contention;
- ownership-edge complexity;
- suspect-buffer growth;
- collection pause and frequency;
- destructor behavior;
- platform allocator differences;
- static versus dynamic runtime parity.

A fast path must not bypass ownership or validation invariants.

## Compiler-specific review

Compiler changes should inspect:

- repeated graph walks;
- clones of AST/HIR/type structures;
- hash-map key and hasher choices;
- diagnostics and source-map allocation;
- generic substitution and monomorphization growth;
- optimization pass order;
- per-module versus monolithic code generation;
- incremental invalidation breadth;
- linker invocation and object reuse.

## LSP performance

Interactive budgets should eventually be defined for:

- incremental diagnostics;
- completion;
- hover;
- go-to-definition;
- rename;
- formatting;
- workspace indexing.

LSP measurements should include small, medium, and large projects and should distinguish cold start from incremental response.

## Benchmark ownership

Each protected benchmark needs:

- name and purpose;
- owner component;
- expected workload;
- baseline platform;
- command;
- acceptable variability;
- regression threshold;
- last verified commit/version.

Obsolete benchmarks should be archived with an explanation rather than silently removed.

## Reporting claims

User-facing performance documentation must distinguish:

- measured result;
- interpretation;
- hypothesis;
- limitation.

Avoid universal claims such as “faster than language X” based on one loop. Prefer specific statements tied to the measured workload.

## CI strategy

Fast CI can run deterministic performance guards that detect severe regressions. Full statistical benchmarks should run on controlled or scheduled environments.

CI should store:

- raw results;
- benchmark version;
- commit and target metadata;
- baseline comparison;
- generated summaries.

A flaky shared runner should not be used to publish precise marketing numbers.