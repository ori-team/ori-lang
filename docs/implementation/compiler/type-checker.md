# Type-checker implementation guide

This guide describes how to extend and refactor Ori semantic checking without changing behavior accidentally.

## Current role

The checker consumes resolved definitions and signatures plus AST modules. It validates semantic rules and records facts needed by lowering.

Its responsibilities include:

- lexical scope and binding state;
- expected and inferred types;
- calls and argument labels/defaults/variadics;
- functions, closures, returns, and control flow;
- structs, enums, patterns, and exhaustiveness;
- traits, implementations, generics, and associated types;
- equality, ordering, indexing, and collection capabilities;
- `using`, destructors, async, tasks, channels, and transferability;
- user-facing semantic diagnostics;
- semantic facts consumed by HIR.

## Design goal

The checker should be understandable as cooperating contexts, not one undifferentiated state object.

Target conceptual split:

```text
Checker
├── ProgramContext        immutable definitions, signatures, module/package context
├── FunctionContext       return, async, iterator, loop, closure state
├── ScopeStack            bindings, mutability, using ownership
├── InferenceContext      inference variables and expected-type propagation
├── TraitContext          constraints, impls, associated items, substitution
├── SemanticFacts         exhaustiveness, captures, cleanup and lowering facts
└── DiagnosticContext     sink, file identity, structured emit helpers
```

This split may begin as grouped fields inside one `Checker` before moving rule families into modules.

## Invariants

- Resolved identities are preferred over repeated textual lookup.
- Recovery types suppress derivative diagnostics.
- Expected types flow into context-typed literals and closures only through documented rules.
- Function/closure/iterator state is restored when leaving nested constructs.
- Scope push/pop is balanced on every path.
- Control-flow facts used by lowering are recorded explicitly.
- Semantic checks do not encode native machine layout.
- Diagnostics use stable codes and current syntax.
- Refactoring preserves diagnostic code, primary span, and accepted/rejected programs unless explicitly changed.

## Recommended module families

```text
check/
├── mod.rs                 public orchestration and Checker façade
├── context.rs             grouped contexts and lifecycle helpers
├── scopes.rs              bindings, lookup, shadowing, capture boundaries
├── expressions.rs         expression inference/checking
├── statements.rs          statement and block checking
├── calls.rs               arguments, labels, defaults, overload-like lookup
├── patterns.rs            bindings, match, destructuring
├── traits.rs              trait/impl/associated behavior
├── generics.rs            substitution, constraints, const arguments
├── equality.rs            equality/hash/order capabilities
├── concurrency.rs         transferability, tasks, channels, async restrictions
├── resources.rs           using/disposal/destructor semantic checks
├── control_flow.rs        returns, loops, exhaustiveness, definite completion
└── diagnostics.rs         semantic diagnostic builders
```

File names are guidance. Split only cohesive responsibilities with stable tests.

## Adding a semantic rule

1. Identify the normative statement and conformance ID.
2. Determine the earliest point where all required type/identity information exists.
3. Add a focused helper with explicit inputs rather than reading unrelated checker state.
4. Return a semantic result or record a fact; avoid emitting and continuing with fabricated valid data.
5. Add a dedicated diagnostic builder when the rule is public and reused.
6. Add positive, negative, interaction, and recovery tests.
7. Review HIR/backend consequences.
8. Update specification, backend matrix, and user docs.

## Expected-type propagation

Expected types may guide:

- anonymous struct/enum shorthand;
- list versus fixed-array literals;
- closure parameter/return context;
- optional/result forms;
- generic argument resolution where specified.

Rules:

- expected types guide checking; they do not silently coerce unrelated types;
- an expression body may still infer independently to produce a precise mismatch;
- empty literals require enough context;
- `try` and other effectful forms follow their specific inference restrictions;
- no new inference path is added only to make one test compile without a language decision.

## Scope and nested-context safety

Nested functions, closures, async bodies, iterators, matches, and loops temporarily change checker state.

Prefer scoped guards or explicit save/restore helpers for:

- current return type;
- current function identity;
- async depth;
- iterator element type;
- loop depth;
- closure capture root;
- transferable-closure depth;
- current constraints and local associated aliases.

A refactor should reduce manual restoration points rather than merely moving them.

## Diagnostics

Diagnostic helpers should receive the semantic data they need:

```rust
fn emit_type_mismatch(
    diagnostics: &mut DiagnosticContext,
    expected: &Ty,
    actual: &Ty,
    span: Span,
)
```

Avoid helpers that read broad mutable checker state when only file identity and type-display context are required.

## Semantic facts for lowering

Record facts when the checker has information lowering cannot reliably reconstruct, such as:

- match exhaustiveness;
- resolved definition identity;
- closure captures and transferability;
- iterator classification;
- selected trait implementation or associated function;
- cleanup/disposal requirements;
- context-resolved type arguments.

Facts should be typed and source-mapped where diagnostics/debugging need them.

## Refactoring sequence

1. Add characterization tests for accepted programs, diagnostics, and spans.
2. Introduce grouped immutable and mutable contexts without moving algorithms.
3. Move one rule family at a time.
4. Replace direct field access with narrow context methods.
5. Remove duplicate lookup/substitution helpers.
6. Run semantic and full integration suites.
7. Measure check latency and allocation.
8. Update crate map and architecture when public boundaries change.

Do not refactor generic substitution, closure capture, and diagnostics simultaneously unless one cannot be separated safely.

## Tests

Minimum focused routes:

```bash
cargo --manifest-path compiler/Cargo.toml test -p ori-types
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test ori_spec
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test method_resolution
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test diagnostic_catalog
```

Also run multifile, concurrency, memory, and LSP tests when the rule crosses those domains.

## Performance review

Watch for:

- repeated linear signature/definition scans;
- cloning large `Ty` structures;
- repeated string construction for diagnostics not emitted;
- recursive substitution without memoization or cycle guards;
- whole-program work inside expression-local checks;
- scope lookup complexity;
- duplicated trait capability checks.

Optimize only after profiling or a protected scalability test.

## Completion checklist

- Same valid/invalid program set unless the contract changed.
- Same stable diagnostic codes and actionable spans.
- No checker state leaks across nested constructs.
- HIR receives all required facts explicitly.
- Focused and workspace tests pass.
- Check-time performance remains within policy.
- Specification and implementation docs agree.