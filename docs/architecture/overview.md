# System architecture overview

Ori is organized as a language toolchain with explicit frontend, semantic, lowering, native-backend, runtime, tooling, and documentation boundaries.

## System context

```text
Ori source and project metadata
        |
        v
Compiler frontend and semantic analysis
        |
        v
Typed HIR and optimization
        |
        v
Cranelift AOT/JIT
        |
        v
Native runtime + platform linker
        |
        v
Executable, shared library, tests, or in-process JIT result
```

The native Cranelift AOT/JIT path is the semantic reference. C source emission was removed under [ADR-0005](../decisions/adr/0005-deprecate-and-retire-c-backend.md); generated C FFI headers and native interoperability remain supported.

## Repository domains

### `compiler/`

Contains the Cargo workspace and compiler crates.

| Crate | Responsibility |
|---|---|
| `ori-lexer` | Source tokenization and lexical diagnostics |
| `ori-ast` | Syntax tree and shared syntax data structures |
| `ori-parser` | Parsing, block structure, recovery, and syntax diagnostics |
| `ori-types` | Definitions, resolution, signatures, type checking, stdlib semantic manifest |
| `ori-hir` | Typed lowering, monomorphization support, and optimization passes |
| `ori-codegen` | Cranelift AOT/JIT, native objects, linking support, generated C FFI headers |
| `ori-runtime` | ARC, cycle collection, collections, strings, I/O, network, tasks, FFI symbols |
| `ori-diagnostics` | Source locations, diagnostic structures, labels, and rendering support |
| `ori-lsp` | Language Server Protocol integration |
| `ori-driver` | CLI and stable orchestration façade across compiler phases |

Dependencies should flow toward lower-level representations and shared contracts. A lower-level crate must not depend on CLI or presentation concerns.

### `stdlib/`

Contains Ori source modules and `.oridoc` sidecars. It complements runtime-backed primitives declared through the semantic stdlib manifest.

### `runtime/`

Contains staged static and dynamic native runtime artifacts by target triple, plus `runtime-link.json` metadata. Source implementation belongs to `ori-runtime`; staged files are distribution inputs.

### `examples/`

Contains executable Ori projects. Examples serve three roles:

- user learning;
- product-surface validation;
- realistic integration coverage.

### `extensions/`

Contains local editor integrations. Extensions consume compiler and LSP contracts; they must not define independent language semantics.

### `tools/`

Contains QA stages, benchmarks, packaging, installers, release scripts, and documentation tooling. Tools should invoke canonical project commands instead of duplicating compiler logic.

### `docs/`

Contains product, architecture, normative specification, implementation, quality, security, governance, planning, operations, and historical evidence.

## Architectural boundaries

### Compiler versus runtime

The compiler owns:

- syntax and semantic validation;
- types, traits, generics, and lowering;
- object and JIT code generation;
- runtime symbol selection and ABI metadata validation.

The runtime owns:

- allocation and ARC operations;
- cycle collection;
- runtime-backed collections and values;
- OS, I/O, networking, concurrency, and process primitives;
- exported native symbols used by generated code.

A rule that can be rejected statically should not be deferred to code generation merely because codegen can detect it.

### Specification versus implementation

The specification defines accepted behavior. Architecture explains how the implementation realizes that behavior. Neither planning documents nor tests alone may redefine the public contract.

### Driver façade

`ori-driver/src/pipeline.rs` is the public orchestration façade. Domain modules own phase-specific inputs, outputs, and implementation details. New features should extend the owning module rather than regrowing a central monolith.

### Native ABI

The ABI boundary connects:

- HIR and codegen layout decisions;
- runtime types and exported symbols;
- staged runtime artifacts;
- `runtime-link.json` metadata;
- embedding headers and shared-library output.

Incompatible changes require an ABI version transition, not only a project-version bump.

## Main data flows

### Check

```text
source/project -> load graph -> lex -> parse -> resolve -> type check -> diagnostics
```

### AOT compile

```text
checked graph -> HIR -> optimize -> native object(s) -> runtime metadata -> linker -> executable/library
```

### JIT run

```text
checked graph -> HIR -> optimize -> Cranelift JIT -> runtime cdylib symbols -> in-process execution
```

### Documentation

```text
source + sidecars + stdlib catalog -> validation -> Markdown/HTML/JSON output -> site/LSP/CLI consumers
```

### LSP

```text
editor document state -> incremental frontend/semantic analysis -> diagnostics/navigation/completion/hover
```

The LSP should reuse compiler semantics and indexes rather than reimplementing language rules.

## Dependency principles

- AST does not contain backend-specific concepts.
- Type checking does not emit machine code.
- HIR is typed and suitable for backend consumption.
- Codegen does not repair invalid source semantics.
- Runtime implementation does not decide source-language syntax.
- CLI presentation does not own compiler contracts.
- Documentation generators consume canonical catalogs and compiler exports where possible.
- Test helpers may depend on public phase contracts but should not create production-only coupling.

## Extension points

Durable extension points include:

- new syntax through the full frontend-to-tooling path;
- new semantic types or traits through type, HIR, backend, runtime, and spec contracts;
- new stdlib operations through the shared manifest and Layer 2/3 source modules;
- new targets through native target metadata and runtime staging;
- new editor features through LSP capabilities;
- new package sources through package resolution interfaces.

Each extension must declare compatibility, security, performance, testing, and documentation impact.

## Evidence

Primary validation includes:

- crate unit tests;
- `ori_spec` conformance tests;
- diagnostic catalog consistency;
- multifile and stdlib integration tests;
- AOT and JIT execution tests;
- memory and ARC tests;
- package and release smoke tests;
- strict Clippy and workspace checks;
- benchmark and performance guards.

See [`../quality/test-strategy.md`](../quality/test-strategy.md) and [`invariants.md`](invariants.md).