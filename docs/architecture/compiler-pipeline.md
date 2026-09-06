# Compiler pipeline

This document defines the current phase model and the responsibilities of each boundary from source loading to execution.

## Goals

The pipeline should make it possible to answer:

- which phase owns a failure;
- which representation is valid at a boundary;
- which diagnostics can be emitted;
- which downstream components must change for a feature;
- which tests prove the phase contract.

## Phase map

```text
project discovery
  -> source graph loading
  -> lexing
  -> parsing
  -> definition collection and name resolution
  -> signature construction and type checking
  -> typed HIR lowering
  -> optimization
  -> AOT object emission or JIT module emission
  -> runtime symbol resolution and linking/execution
```

Documentation, formatter, migration, summary, doctor, and LSP routes reuse selected portions of this pipeline.

## 1. Project discovery and source loading

**Owner:** `ori-driver` project pipeline and package/project modules.

**Inputs:** source path or project directory, `ori.proj`, package manifests, lockfile, environment overrides.

**Outputs:** resolved project context, dependency scopes, source graph, stdlib root, entry module.

**Responsibilities:**

- locate the project root and entry file;
- parse and validate project/package metadata;
- resolve path, Git, and registry dependencies;
- enforce package namespace boundaries;
- load imports recursively;
- detect missing modules and cycles;
- validate module declarations against source paths where required;
- produce stable source identities and dependency context.

**Must not:** perform type checking or hide stale lockfile and dependency errors.

## 2. Lexing

**Owner:** `ori-lexer`.

**Input:** UTF-8 source text and file identity.

**Output:** token stream with spans and lexical diagnostics.

**Responsibilities:**

- recognize canonical tokens and trivia;
- preserve enough span information for diagnostics and formatting;
- reject invalid characters and malformed literals;
- remain independent of project and backend behavior.

**Evidence:** lexer unit tests, invalid-source conformance cases, fuzzing targets.

## 3. Parsing

**Owner:** `ori-parser`, with syntax structures in `ori-ast`.

**Input:** token stream and source text.

**Output:** AST plus syntax diagnostics.

**Responsibilities:**

- implement the normative grammar;
- enforce block structure and canonical S3 spellings;
- produce accurate spans;
- recover sufficiently to report additional useful errors without inventing valid semantics;
- reject removed syntax with dedicated diagnostics where required;
- maintain formatter-compatible structure.

**Important invariants:**

- token helpers consume trivia consistently;
- parser state is local to a source file;
- precedence and associativity are centralized and tested;
- block termination and optional end labels are consistent across constructs.

## 4. Definitions and name resolution

**Owner:** `ori-types` resolution modules.

**Input:** AST and project/module context.

**Output:** definition map, signatures, import/re-export relationships, resolved identities, name diagnostics.

**Responsibilities:**

- assign stable definition identities;
- enforce duplicate, private, missing, and ambiguous name rules;
- resolve imports without crossing package boundaries accidentally;
- build type, function, trait, implementation, and value signatures;
- preserve enough information for type checking and HIR lowering.

**Must not:** encode machine layouts or linker behavior.

## 5. Type checking and semantic analysis

**Owner:** `ori-types` checker.

**Input:** AST, definitions, signatures, project and stdlib semantic context.

**Output:** checked semantic facts, inferred local types, exhaustiveness and return facts, semantic diagnostics.

**Responsibilities:**

- validate assignments, calls, control flow, visibility, traits, generics, and constraints;
- implement documented local inference rules;
- validate transferability, disposability, equality, ordering, and other semantic capabilities;
- record facts required by lowering instead of forcing later phases to re-derive them;
- suppress cascaded diagnostics using explicit error types;
- keep diagnostic messages aligned with current syntax and terminology.

**Refactoring direction:** checker state should be grouped into explicit program, function, scope, inference, trait, and diagnostic contexts without changing public semantics.

## 6. Typed HIR lowering

**Owner:** `ori-hir` and the driver lowering pipeline.

**Input:** checked modules, definitions, signatures, semantic facts, stdlib metadata.

**Output:** typed HIR modules and backend-ready program structure.

**Responsibilities:**

- remove syntax-only distinctions;
- lower expressions, statements, traits, generics, iterators, async behavior, and cleanup into explicit typed forms;
- prepare monomorphized or specialization-ready structures;
- preserve source mapping and debug information;
- split native modules where the incremental pipeline requires it.

**Must not:** receive unresolved source-language errors as ordinary valid HIR.

## 7. Optimization

**Owner:** `ori-hir::optimize`.

**Input:** valid typed HIR.

**Output:** semantically equivalent optimized HIR.

**Current families include:** constant folding, strength reduction, dead-code elimination, leaf inlining, and pipeline orchestration.

**Requirements:**

- each pass states preconditions and preserved invariants;
- optimization level selection is explicit;
- pass order is documented and tested;
- performance improvements are measured;
- disabled optimization remains a reference path for differential testing;
- no pass changes observable language behavior.

### Scalar argument temporaries

At `ORI_OPT=aggressive`, the existing leaf inliner can materialize arguments
when a same-module direct call is the complete value of `Let`, `Return`, or
`Expr`. This fallback applies when conservative expression substitution declines
the call. Numeric and boolean parameter/return types are eligible; variadics,
parameter contracts, async functions, closure captures, managed signatures,
propagation, await, and binding-bearing return expressions remain excluded.

Every argument becomes an immutable typed HIR `Let`, in source order, including
unused arguments. The return expression receives simultaneous substitutions of
fresh variable references. DCE may remove unused pure bindings, but preserves
calls and potentially trapping division, remainder, shifts, and indexing.

Temporary names use `$ori.inline.N`. The lexer identifier grammar cannot contain
`$` or `.`, so source locals, parameters, and globals cannot occupy this namespace.
This pass is its only producer; it reserves existing temporary indices recursively
before assigning module-wide indices, including on repeated optimizer runs.

Only statement-owned blocks receive these bindings. No temporary is hoisted from
nested calls, expression branches, short-circuit operands, loop conditions,
match guards, or assignments (including effectful lvalues). Eligible statements
inside a branch or loop body remain inside that block. The previous conservative
expression path and its compound-global/call guard remain intact. Native AOT and
JIT consume the same HIR; no runtime or ABI contract changes.

Structural regressions live in `inline_leafs.rs`; execution and optimization-level
parity live in `arithmetic_and_optimizer_guards.rs` and `jit_run.rs`. This is not
ownership-aware general inlining or a claim of universal performance improvement.

## 8. Native AOT code generation

**Owner:** `ori-codegen` plus the driver compile pipeline.

**Input:** optimized typed HIR, target, compile options, runtime metadata.

**Output:** native object files, shared-library headers, debug sidecars, linked executable or library.

**Responsibilities:**

- map Ori types and control flow to Cranelift IR;
- emit correct ownership and cleanup operations;
- preserve ABI layouts and symbol naming;
- emit per-module or monolithic objects according to safe incremental rules;
- resolve linker strategy and platform libraries;
- produce debug information and generated C headers where requested.

**Must not:** reinterpret unsupported semantics. Unsupported operations must fail explicitly before or during backend validation with a stable diagnostic.

## 9. JIT execution

**Owner:** Cranelift JIT support in `ori-codegen` and driver native execution pipeline.

**Input:** optimized HIR and packaged runtime cdylib.

**Output:** in-process program result.

**Responsibilities:**

- resolve `ori_*` runtime symbols from the correct cdylib;
- keep AOT and JIT semantics aligned;
- avoid temporary object and linker requirements on the JIT route;
- preserve cleanup and panic/error behavior;
- provide testable output and exit behavior.

A runtime source change requires static and dynamic staged artifacts to stay synchronized.

## 10. Runtime and link boundary

**Owner:** `ori-runtime`, `runtime/` artifacts, driver runtime/link modules.

**Responsibilities:**

- provide ABI-versioned exported symbols;
- implement allocation, ARC, collections, strings, I/O, network, tasks, and other runtime primitives;
- expose static and dynamic artifacts for supported targets;
- validate project version, ABI version, target, and required libraries through metadata;
- keep platform-specific details isolated.

See [`runtime-and-memory.md`](runtime-and-memory.md).

## Auxiliary routes

### Formatter

Consumes source and syntax structure, emits canonical source, and must be idempotent and semantics-preserving.

### Documentation

Consumes source comments, `.oridoc` sidecars, stdlib signatures, diagnostic catalogs, and keywords. Documentation validation is a compiler-supported workflow, not a separate language implementation.

### LSP

Reuses lexer, parser, resolution, checker, formatter, and stdlib catalogs. LSP-only semantic rules are prohibited.

### Migration

`ori migrate-syntax` performs best-effort mechanical rewrites. It does not guarantee semantic migration and must not silently produce invalid current syntax.

### Debugger

Consumes compiler-generated debug maps and cooperative runtime hooks. Debugger metadata observes program structure without changing language semantics.

## Feature impact matrix

| Feature kind | Minimum phases to inspect |
|---|---|
| New token or syntax | lexer, AST, parser, formatter, migration, LSP, spec, conformance |
| New type rule | AST if needed, resolution, checker, HIR, backend, diagnostics, spec, tests |
| New runtime-backed operation | stdlib manifest, checker signature, HIR/ABI, runtime, AOT/JIT, docs, LSP, tests |
| New optimization | HIR pass, pass pipeline, differential tests, benchmarks, debug mapping |
| New CLI command | driver façade, command module, help, docs, integration tests, packaging if needed |
| New target | target model, codegen, link metadata, runtime staging, CI, release docs |

## Phase-contract review

A phase-boundary change requires:

- explicit input/output type changes;
- affected callers and consumers;
- compatibility and incremental-cache impact;
- diagnostic ownership;
- test updates at the phase and integration layers;
- architecture update;
- an ADR when the boundary is durable and non-obvious.