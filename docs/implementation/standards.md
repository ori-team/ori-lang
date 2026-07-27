# Implementation standards

These standards apply to production code in the Ori repository. They are intended to preserve compiler correctness, visible contracts, and maintainability rather than impose generic application architecture.

## 1. Design from contracts

Before implementation, identify:

- the owning product and normative documents;
- the component and phase that own the behavior;
- input and output contracts;
- invariants;
- compatibility and migration effects;
- diagnostic ownership;
- test and evidence requirements;
- runtime, ABI, security, performance, LSP, packaging, and documentation impact.

Do not begin by editing the first place where a failing symptom appears.

## 2. Deliver vertical behavior

A PR should deliver a coherent behavior through every affected layer.

For a language feature this may include:

```text
syntax -> AST -> resolution -> type checking -> HIR -> backend/runtime
       -> formatter/LSP -> diagnostics -> tests -> specification -> changelog
```

Avoid horizontal PRs that add unused abstractions without proving a real vertical use case.

## 3. Keep phase boundaries explicit

Each phase should expose:

- a focused input type;
- a focused output type;
- diagnostics or typed errors;
- documented preconditions and postconditions.

Prefer a domain input object over functions with many unrelated parameters.

```rust
struct CheckInput<'a> {
    source: &'a SourceFile,
    definitions: &'a DefMap,
    signatures: &'a SignatureIndex,
    options: CheckOptions,
}

struct CheckOutput {
    facts: SemanticFacts,
    diagnostics: Vec<Diagnostic>,
}
```

Use this pattern when it improves a real boundary. Do not create wrapper types that merely rename one argument.

## 4. Use small façades

A façade coordinates stable entry points and re-exports domain outputs. It does not accumulate every implementation detail.

The `ori-driver` pipeline façade is the preferred model:

- modules own phase-specific behavior;
- result types belong to the domain module;
- cross-stage policy stays minimal and visible;
- CLI and LSP call stable entry points.

When a façade grows new parsing, rendering, linking, loading, or domain algorithms, move the behavior to the owning module.

## 5. Group state by responsibility

Large state objects should group related concerns into explicit contexts.

For example, checker state can be separated conceptually into:

- immutable program context;
- current function/control-flow context;
- scope stack;
- inference state;
- trait/generic state;
- diagnostic sink;
- recorded semantic facts.

The first refactor may only group fields and introduce accessors. Do not combine structural refactoring with new language semantics.

## 6. Put validation in the earliest informed phase

- Lexical shape belongs in the lexer.
- Grammar belongs in the parser.
- Symbol and visibility rules belong in resolution.
- Type, trait, and control-flow rules belong in semantic analysis.
- Representation and target constraints belong in lowering/backend validation.
- Runtime-only state and OS failures belong in the runtime.

Codegen must not repair invalid source. Runtime checks must not replace precise compile-time diagnostics where the compiler has enough information.

## 7. Prefer typed errors internally

Public user failures should become structured diagnostics or Ori `result` values.

Internal APIs should prefer typed error enums when callers need to distinguish causes. `Result<T, String>` is acceptable at narrow presentation boundaries or during migration, but it should not become the default domain contract.

Error types should carry data. Presentation code decides final wording.

## 8. Treat diagnostics as public API

- Use registered stable codes.
- Keep one semantic meaning per code.
- Provide a precise primary span.
- Add cause and action when they improve recovery.
- Use current syntax in suggestions.
- Avoid duplicate follow-on errors after an earlier recovery value.
- Add catalog and test coverage in the same change.

See [`../quality/diagnostic-design.md`](../quality/diagnostic-design.md).

## 9. Isolate `unsafe`

Raw-pointer and FFI functions should be thin adapters:

```text
validate raw input -> convert to typed representation
-> call safe domain logic -> convert result to ABI form
```

Every `unsafe` block or function must have a reviewable safety argument covering applicable points:

- pointer provenance and nullability;
- alignment;
- initialized memory;
- valid length and bounds;
- lifetime;
- aliasing and mutability;
- ownership and retain/release behavior;
- thread safety;
- unwind behavior across FFI;
- ABI layout and calling convention.

See [`../security/unsafe-code-policy.md`](../security/unsafe-code-policy.md).

## 10. Preserve ABI and symbol contracts

When touching runtime or codegen:

- do not rename or change `#[no_mangle]` exports accidentally;
- preserve documented layouts and tags;
- validate static and dynamic runtime artifacts;
- update Spec 19 for contract changes;
- bump the ABI tag for incompatible changes;
- test generated headers and runtime metadata;
- inspect target-specific link behavior.

Moving code between Rust modules is acceptable only when exported symbol names and behavior remain unchanged.

## 11. Use one declarative source where drift is expensive

Prefer shared typed catalogs for data consumed by several crates, such as:

- stdlib operations and ABI metadata;
- diagnostic codes;
- target capabilities;
- backend-support matrices;
- document catalog and generated indexes.

Generated output must remain inspectable. A generator should eliminate duplication, not hide semantics.

## 12. Visibility and dependencies

- Keep Rust visibility as narrow as practical.
- Avoid exposing internal representation solely for tests; prefer dedicated test helpers under controlled visibility.
- Lower-level crates must not depend on CLI or editor presentation.
- Avoid cycles and backchannels through global state.
- New cross-crate dependencies require a clear architectural reason.
- Shared crates should contain stable concepts, not miscellaneous helpers.

Do not create generic `utils` modules for unrelated functions. Name modules by domain or responsibility.

## 13. Function and module size

There is no universal line limit.

Refactor when a file or function:

- owns several independent responsibilities;
- requires unrelated state;
- changes for unrelated reasons;
- is difficult to test without broad setup;
- hides phase transitions;
- repeats safety or ownership logic;
- produces review diffs that are hard to reason about.

Do not split a cohesive algorithm merely to satisfy a number.

## 14. `unwrap`, panic, and abort policy

`unwrap` or `expect` is acceptable when:

- a local check immediately proves the condition;
- the condition is a documented internal invariant;
- the code is a test;
- the message makes invariant failure actionable.

It is not acceptable for recoverable user-controlled input such as:

- source files;
- manifests and lockfiles;
- filesystem and network operations;
- registry responses;
- package archives;
- environment values;
- runtime handles that can be invalid by contract.

Use abort only for documented unrecoverable runtime corruption, bounds traps, or ABI violations where continuing is unsafe. Panics must not unwind across FFI boundaries.

## 15. Comments and documentation in code

Comments should explain:

- invariants;
- ownership and lifecycle;
- non-obvious algorithms;
- platform constraints;
- safety arguments;
- why an apparently simpler approach is wrong;
- links to normative contracts or ADRs.

Do not narrate obvious control flow. Do not leave temporary planning history in source comments after the design is documented elsewhere.

Public APIs need Rust documentation when they form a crate or phase contract.

## 16. Testing rules

Every change should use the lowest useful test layer plus the user-visible layer when behavior crosses boundaries.

Required patterns:

- bug: reproduce first, then fix;
- diagnostic: negative test plus catalog consistency;
- language feature: positive, negative, formatter/tooling, and backend evidence;
- runtime ownership: focused runtime test plus AOT/JIT integration;
- optimization: semantic differential test plus benchmark;
- refactor: characterization and parity tests before moving behavior;
- package/release: isolated smoke test outside the workspace.

Avoid tests that only assert internal source text unless they guard a deliberate architectural boundary and no stronger behavioral test is practical.

## 17. Performance rules

- Measure before optimizing.
- Preserve a no-optimization or reference path for differential testing where practical.
- Report workload, build mode, target, sample count, and variability.
- Avoid optimizing microbenchmarks in ways that remove the intended work.
- Do not accept large complexity increases for unmeasured gains.
- Runtime hot paths require allocation, retain/release, contention, and realistic workload consideration.

## 18. Refactoring policy

Safe refactoring sequence:

1. document the current contract and invariants;
2. add characterization tests;
3. extract one cohesive responsibility;
4. preserve public APIs, symbols, diagnostics, and layouts;
5. run focused and broad gates;
6. measure performance where relevant;
7. update architecture and implementation docs;
8. repeat in another small PR.

Do not combine broad runtime/checker restructuring with syntax or semantic changes.

## 19. Documentation with code

Update in the same PR when applicable:

- normative specification;
- architecture;
- implementation guide;
- diagnostic catalog;
- examples and user guide;
- compatibility/versioning policy;
- security/threat model;
- operations/release procedure;
- changelog;
- ATLAS and machine catalog for new canonical docs.

## 20. Review checklist

Before merge, reviewers should be able to answer:

- Is the owning phase correct?
- Are inputs, outputs, and errors explicit?
- Are invariants preserved?
- Is there a competing source of truth?
- Can invalid input reach `unsafe` or codegen unexpectedly?
- Are AOT, JIT, C/debug, LSP, formatter, docs, and packages affected?
- Are tests strong enough to catch the original defect or contract drift?
- Is the diff one coherent unit?
- Is any new abstraction justified by current use?
- Are compatibility and release effects explicit?