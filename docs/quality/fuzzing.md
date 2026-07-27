# Fuzzing strategy

Fuzzing complements deterministic conformance and regression tests by exploring malformed, unexpected, and adversarial inputs.

## Goals

- prevent compiler crashes on hostile source/project data;
- improve parser and diagnostic recovery;
- find span, length, overflow, and allocation defects;
- exercise runtime ownership state transitions;
- convert every useful finding into a minimized permanent regression.

## Priority targets

### Lexer

Inputs:

- arbitrary bytes decoded or rejected according to UTF-8 policy;
- Unicode boundaries;
- numeric and string literal extremes;
- comments, escapes, and unterminated forms;
- very long tokens.

Properties:

- no panic or out-of-bounds access;
- spans remain inside source bounds;
- deterministic token/diagnostic result;
- invalid UTF-8 handling follows the source contract.

### Parser

Inputs:

- arbitrary token streams from the lexer;
- deeply nested blocks/types/patterns;
- incomplete editor documents;
- removed and mixed syntax;
- adversarial newline/trivia placement.

Properties:

- no panic or unbounded recursion for configured limits;
- every emitted span is valid;
- recovery terminates;
- deterministic diagnostics;
- no fabricated valid module from irrecoverable input.

### Formatter

Inputs:

- parser-accepted source;
- partially invalid source where formatting is supported;
- comments and pathological whitespace.

Properties:

- idempotence;
- formatted output parses when input is valid;
- semantic structure is preserved;
- output growth is bounded relative to input.

### Manifests, lockfiles, and packages

Inputs:

- malformed schemas;
- unknown versions;
- path traversal;
- duplicate dependencies;
- extreme lengths/counts;
- archive entries and metadata.

Properties:

- no path escape;
- no command injection;
- no panic;
- deterministic rejection;
- resource limits;
- credentials are not echoed.

### Runtime values and parsers

Targets:

- strings/bytes/UTF-8/hex/JSON;
- collection index/slice bounds;
- network address and protocol parsing;
- result/optional/tag decoding;
- generated embedding inputs.

Properties:

- invalid input returns a documented error or trap;
- no undefined behavior;
- lengths and allocation arithmetic are checked;
- arbitrary bytes are not treated as C strings.

### ARC and cycle collection

Use model-based operation sequences:

- allocate;
- retain/release;
- register/update/remove edge;
- create cycles;
- run collection;
- invoke destructor/finalization;
- transfer through wrappers/collections.

Properties:

- no negative reference count;
- no double finalization/free;
- no stale edge/suspect reference;
- reachable objects survive;
- unreachable cycles are reclaimed;
- model and runtime live-object sets agree.

## Harness rules

- Keep target setup minimal.
- Separate parser crashes from semantic/codegen targets.
- Bound recursion, input size, subprocess time, and generated artifacts.
- Reset or isolate process-global state.
- Make environment and target assumptions explicit.
- Preserve deterministic seeds for CI smoke runs.
- Do not run arbitrary produced native binaries without a containment and timeout strategy.

## Corpus

Seed corpora should include:

- current examples;
- normative specification snippets;
- invalid diagnostic fixtures;
- historical removed syntax;
- minimized prior bugs;
- boundary-value manifests and runtime inputs.

Corpus files need a purpose or provenance note. Remove duplicates that add no coverage.

## Finding workflow

1. Preserve the failing input and fuzzer metadata.
2. Reproduce outside the fuzzer.
3. Minimize the input.
4. Classify phase, impact, and security relevance.
5. Add a deterministic regression at the lowest useful layer.
6. Fix the defect.
7. Add integration/conformance evidence when public behavior is involved.
8. Record security disclosure privately when appropriate.

A fuzzer corpus entry alone is not an adequate permanent regression.

## CI and scheduled execution

- PR CI may run short deterministic corpus/smoke checks.
- Scheduled or dedicated runners perform longer fuzz campaigns.
- Crashes and timeouts publish reproducible artifacts without secrets.
- Fuzz infrastructure version and seed are recorded.
- Known failures require issues and must not be silently ignored.

## Tooling direction

Rust fuzzing may use `cargo-fuzz`/libFuzzer or another justified engine. Property generators can complement coverage-guided fuzzing.

The exact tool is less important than:

- stable harness APIs;
- reproducibility;
- sanitizer compatibility;
- minimized regression conversion;
- coverage of real trust boundaries.

## Security

Potential memory corruption, path escape, token leakage, or remote-input crash findings follow `SECURITY.md` and the threat model. Do not post exploitable details publicly before triage.

## Completion criteria for QA-FUZZ-1

- documented commands and prerequisites;
- lexer and parser targets;
- formatter property target;
- at least one manifest/package or runtime parser target;
- ARC model target or staged implementation plan;
- seed corpus and minimized-regression policy;
- scheduled execution route;
- failure artifact and triage procedure.