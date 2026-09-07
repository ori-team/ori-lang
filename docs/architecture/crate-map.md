# Compiler crate map

The Cargo workspace under `compiler/` is divided into crates that represent compiler phases or stable shared services.

## Dependency direction

Conceptual direction:

```text
source foundations
  ori-diagnostics
  ori-lexer
  ori-ast
        |
        v
  ori-parser
        |
        v
  ori-types
        |
        v
  ori-hir
        |
        v
  ori-codegen <-> ori-runtime ABI contract
        |
        v
  ori-driver
        |
        +--> ori-lsp and CLI/tooling consumers
```

This diagram is conceptual, not a replacement for Cargo metadata. Dependencies should follow phase/data ownership and avoid circular semantic ownership.

## `ori-diagnostics`

**Owns:** file identities, spans, labels, diagnostic structure, severity, and rendering-support data.

**Must not own:** parser/type rules, CLI policy, or editor-specific semantics.

**Important consumers:** every phase, driver, LSP.

**Evidence:** diagnostic unit tests, catalog parity, CLI/LSP integration.

## `ori-lexer`

**Owns:** tokenization, lexical trivia, literal scanning, token spans, lexical diagnostics.

**Input:** source text and file identity.

**Output:** tokens and lexical diagnostics.

**Must not own:** module resolution, type semantics, backend behavior.

## `ori-ast`

**Owns:** syntax-tree data structures shared by parser, formatter, semantic analysis, and source-oriented tooling.

**Must preserve:** source spans and syntax distinctions required by current tools.

**Must not contain:** Cranelift types, runtime symbols, linker options, CLI output.

## `ori-parser`

**Owns:** grammar implementation, precedence, block structure, syntax recovery, removed-form diagnostics.

**Uses:** lexer tokens, AST nodes, diagnostics.

**Output:** source-file AST and syntax diagnostics.

The parser core owns token navigation and trivia behavior; focused modules own item, statement, expression, pattern, and type productions.

## `ori-types`

**Owns:** definitions, name resolution, signatures, type representation, checking, semantic capabilities, stdlib semantic catalog.

**Key responsibilities:**

- stable `DefId` identities;
- visibility and imports;
- function/type/trait/implementation signatures;
- local inference;
- assignability and constraints;
- exhaustiveness and control-flow facts;
- transferability and disposal checks;
- stdlib path/signature/ABI metadata.

**Refactoring direction:** separate immutable program indexes from current checking state and split rule families without changing semantics.

## `ori-hir`

**Owns:** typed high-level intermediate representation, lowering support, generic preparation, and semantic-preserving optimization passes.

HIR should expose behavior needed by code generation explicitly and discard syntax-only detail that no longer matters.

**Optimization families:** constant folding, dead-code elimination, strength reduction, inlining, pipeline orchestration, and future measured passes.

## `ori-codegen`

**Owns:** native and JIT code generation, object emission, target-aware layout lowering, linking support, generated C FFI headers, and debug symbols.

**Boundary:** consumes valid typed HIR and runtime/ABI metadata. It does not define source-language validity.

**Critical concerns:** ownership emission, cleanup, symbol references, target layouts, AOT/JIT parity, incremental object boundaries.

## `ori-runtime`

**Owns:** native exported operations, managed allocation, ARC, cycle collection, runtime values and collections, I/O, networking, process, concurrency, test/debug hooks.

**Boundary:** supplies `ori_*` symbols and ABI-defined layouts to generated code.

**Refactoring direction:** thin FFI exports over safe domain modules while preserving symbols and layouts.

## `ori-driver`

**Owns:** stable orchestration façade and user-facing command workflows.

Domain modules own:

- project/source loading;
- frontend entry points;
- lowering;
- compile/link;
- JIT/test execution;
- formatter;
- docs;
- reports/doctor/summary;
- migration;
- debug workflows.

The façade re-exports focused input/output contracts. It must not regrow domain algorithms.

## `ori-lsp`

**Owns:** Language Server Protocol state and translation between editor requests and compiler semantic services.

**Must reuse:** lexer, parser, semantic analysis, formatter, stdlib catalog, and diagnostics.

**Must not own:** a parallel grammar, type system, or stdlib signature table.

## Cross-crate contract rules

- Shared data belongs in the lowest crate that conceptually owns it.
- A lower phase does not depend on driver, CLI, LSP, or presentation.
- Runtime and codegen coordinate through normative ABI contracts, not Rust module visibility alone.
- Test-only dependencies should not leak into production architecture.
- New dependencies require a reason in the PR and architecture update when they establish a durable edge.
- Generic “shared utils” crates are avoided unless the contents form a real stable domain.

## Public Rust APIs

A crate public API should represent one of:

- phase input/output contract;
- stable shared representation;
- diagnostic/service interface;
- documented runtime/codegen bridge.

Keep visibility narrow for implementation helpers. Do not expose private structures only to make a test convenient.

## Change-impact examples

| Change | Primary crates | Common downstream review |
|---|---|---|
| New token/syntax | lexer, AST, parser | types, HIR, formatter, LSP, driver |
| New type feature | AST, types | HIR, codegen, runtime, LSP |
| New optimization | HIR | codegen, debug info, benchmarks |
| Runtime operation | types catalog, runtime | HIR/codegen, driver, LSP/docs |
| New command | driver | operations, packaging, LSP only if shared |
| Diagnostic structure | diagnostics | every emitting phase, CLI, LSP, docs export |

## Validation

Crate-boundary changes should run:

- affected crate unit tests;
- downstream compile/check;
- focused phase tests;
- `ori_spec` or integration tests when semantics cross boundaries;
- strict Clippy gate;
- workspace test;
- architecture/invariant review.

Cargo's actual dependency graph should eventually be exported and compared against an allowed architecture graph.