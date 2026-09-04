# Test strategy

Ori uses layered tests so failures can be localized while public behavior remains protected end to end.

## Test principles

1. Test the contract at the lowest useful layer.
2. Add an integration test when behavior crosses a phase or becomes user-visible.
3. Reproduce bugs before fixing them.
4. Prefer deterministic tests.
5. Keep fixtures minimal and named after the behavior they prove.
6. Test invalid programs as deliberately as valid programs.
7. Do not claim backend support without execution or explicit rejection evidence.
8. Refactoring requires characterization tests before code movement.

## Layers

### Unit tests

Own local algorithms and invariants:

- lexer tokenization;
- parser productions and recovery helpers;
- type substitution and semantic predicates;
- HIR transformations;
- codegen layout helpers;
- runtime data structures and ownership operations;
- diagnostic rendering helpers.

Unit tests should avoid rebuilding the full toolchain when a focused API proves the rule.

### Phase tests

Validate an entire compiler phase with realistic inputs and outputs:

- source → tokens;
- tokens → AST;
- AST/signatures → diagnostics;
- checked program → HIR;
- HIR → optimized HIR;
- HIR → machine object or JIT module.

Phase tests should assert stable contracts, not private implementation sequence.

### Conformance tests

`ori_spec` and related fixtures prove normative language behavior. Each important specification rule should have a stable conformance identifier and positive/negative evidence.

See [`language-conformance.md`](language-conformance.md).

### Driver integration tests

Validate public commands and cross-phase behavior, including:

- check;
- run;
- compile;
- test;
- formatter;
- documentation;
- doctor and summary;
- packages, lockfiles, and incremental output;
- debugger metadata;
- migration.

### Runtime and memory tests

Validate:

- header layouts;
- retain/release accounting;
- edge registration and removal;
- cycle collection;
- destructor order and single execution;
- managed collection ownership;
- async cleanup;
- task/channel transferability;
- I/O and resource closure;
- static/cdylib symbol parity.

### AOT/JIT differential tests

For shared support, compile and execute the same program through AOT and JIT and compare:

- stdout/stderr;
- exit behavior;
- returned values;
- diagnostics;
- cleanup effects;
- resource behavior.

### C/debug backend tests

The C/debug backend has two valid outcomes:

- behavior matches the documented supported subset;
- unsupported behavior is rejected explicitly.

Silent semantic divergence is a failure.

### LSP tests

Validate diagnostics, hover, navigation, rename, completion, semantic tokens, formatting, stdlib help, and incremental synchronization through the public protocol boundary.

### Packaging and release tests

Release packages must be tested outside the workspace with developer-only paths unavailable.

Validate:

- executable and LSP presence;
- runtime static library and cdylib;
- stdlib source;
- runtime metadata;
- version and ABI agreement;
- JIT without Rust;
- AOT with documented platform prerequisites;
- installers and uninstallers;
- archive extraction and checksums.

## Required evidence by change type

| Change | Minimum evidence |
|---|---|
| Lexer/parser bug | focused unit/phase regression + public diagnostic test |
| Type rule | positive + negative semantic tests + conformance mapping |
| Runtime ownership | runtime test + AOT + JIT + memory suite |
| New stdlib operation | semantic signature + native execution + docs/LSP parity |
| Optimization | semantic differential + benchmark |
| CLI behavior | command integration test + help/docs update |
| Package format | parser/resolver tests + isolated package smoke |
| Refactor | characterization + focused parity + broad gate |
| Diagnostic wording | golden/catalog test + accessibility review |

## QA stages

The repository currently uses staged QA scripts under `tools/qa/`. The detailed mapping remains in `docs/planning/qa/test-matrix-ori.md` during migration.

Fast daily gate:
- `sh tools/qa/daily_fast.sh` (D0 atlas, metadata, ABI exports; S0 check/clippy; S1 unit tests; S2 spec + catalog; S3 memory/security; S8 residual surface).

Sanitizer smoke:
- `sh tools/qa/sanitizer_smoke.sh` (AddressSanitizer and ThreadSanitizer execution for runtime and embed targets).

Typical progression:

```text
S0 workspace and strict static checks
S1 frontend/unit tests
S2 specification and diagnostic catalog
S3 memory, security, concurrency
S4 multifile, stdlib, package boundaries
S5 full workspace
S6 examples and product surface
S7 performance
S8 backend residual/support gates
```

The script is the operational source for exact commands; this document owns the strategy and evidence model.

## Fuzzing

Priority targets:

- lexer arbitrary UTF-8 and byte boundaries;
- parser arbitrary token streams and incomplete editor input;
- formatter parse/format round trips;
- manifest, lockfile, package archive, and registry decoding;
- diagnostic renderer span boundaries;
- runtime ownership-operation sequences;
- bytes, strings, JSON, and network parsers.

Fuzz findings become minimized regression fixtures.

## Property-based tests

Useful properties include:

- `format(format(source)) == format(source)`;
- parse → format → parse preserves semantic structure;
- optimized and unoptimized programs agree;
- AOT and JIT agree;
- retain/release sequences never produce negative ownership or duplicate free;
- collection operations match a simple Rust model;
- serialization round trips preserve valid manifests and lockfiles;
- deterministic inputs produce deterministic diagnostics and metadata.

## Test isolation

Tests must control:

- temporary directories;
- environment variables;
- runtime staging paths;
- process-global caches;
- ports and network timeouts;
- locale;
- current working directory;
- generated artifacts.

Global runtime or driver state needs reset helpers or process isolation where shared state can leak between tests.

## Golden and snapshot policy

Golden files are appropriate for:

- diagnostics;
- formatted source;
- generated headers;
- documentation output;
- debug metadata;
- stable CLI output.

A golden update must be reviewed as a behavior change. Do not bulk-accept snapshots without understanding the diff.

## Coverage

Line coverage is useful for finding untested regions but does not measure language conformance.

Track separately:

- Rust line/branch coverage;
- specification rule coverage;
- diagnostic-code coverage;
- backend support coverage;
- target/platform coverage;
- stdlib operation coverage;
- example and package coverage.

## Flaky tests

A flaky test is a defect. Quarantine is temporary and requires:

- an issue;
- an owner;
- failure evidence;
- a removal condition;
- no use as an excuse to ignore a failing release gate.

## Completion reporting

A PR should list exact commands and outcomes. “Tests pass” is insufficient when only a subset was run. State skipped gates and the reason.