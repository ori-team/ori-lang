# Standard-library architecture

Ori's standard library combines runtime-backed primitives with source modules written in Ori.

## Layer model

| Layer | Location | Responsibility |
|---|---|---|
| Layer 1 | `ori-types` manifest + `ori-runtime` | ABI-backed primitives, hot paths, OS integration, managed runtime services |
| Layer 2 | `stdlib/**/*.orl` | Safe wrappers and compositional APIs written in Ori |
| Layer 3 | `stdlib/**/*.orl` | Pure algorithms built on lower layers |
| Documentation | `stdlib/**/*.oridoc`, Spec 12, generated exports | Public signatures, behavior, examples, editor help |

Layer numbers describe responsibility, not user-visible namespaces. Public APIs should present coherent `ori.*` modules.

## Canonical sources

- Semantic path, aliases, runtime symbol, and backend flags: `compiler/crates/ori-types/src/stdlib.rs`
- Native implementation: `compiler/crates/ori-runtime/`
- Ori source implementation: `stdlib/**/*.orl`
- Sidecar docs: `stdlib/**/*.oridoc`
- Normative public contract: `docs/spec/12-stdlib.md`
- Maintenance flow: `docs/spec/15-stdlib-maintenance.md`
- Current module inventory: `stdlib/README.md`

No downstream crate may maintain a competing full stdlib registry.

## Runtime-backed operation flow

```text
canonical path
  -> semantic signature
  -> native ABI metadata
  -> runtime symbol
  -> backend declaration/resolution
  -> LSP/docs export
  -> conformance and execution tests
```

A new operation is incomplete when any link is missing.

## Source-module discovery

An Ori module path maps to an `.orl` path under `stdlib/`. The file declares the corresponding `module ori...` identity and follows ordinary public/private rules.

The project loader may load source modules when selected imports or module discovery require them. Runtime-backed parent modules remain lightweight where the current import contract requires it.

## Ownership of behavior

- Type and call rules belong in the semantic manifest/checker.
- ABI shape belongs in the shared stdlib ABI metadata and Spec 19.
- Native hot-path behavior belongs in `ori-runtime`.
- Pure composition belongs in `.orl` modules.
- User-visible behavior belongs in Spec 12 and sidecar docs.
- Editor help derives from the same signatures and docs.

A backend-specific patch must not become a second definition of the stdlib contract.

## Adding a runtime-backed function

The vertical change normally includes:

1. canonical path and aliases;
2. semantic signature;
3. native ABI metadata;
4. exported runtime symbol;
5. AOT/JIT symbol visibility;
6. backend-support classification;
7. `.oridoc` or source documentation;
8. LSP/doc export visibility;
9. positive type-check test;
10. negative semantic test where relevant;
11. native execution test;
12. C/debug test or explicit unsupported classification;
13. Spec 12/15 update;
14. changelog entry when user-visible.

## Adding a Layer 2 or Layer 3 function

- place the implementation in the canonical `stdlib/` module;
- use current S3 syntax and public visibility where exported;
- call Layer 1 through normal imports rather than hidden compiler shortcuts;
- avoid duplicating a runtime primitive in Ori without a documented reason;
- add executable multifile coverage;
- update sidecar docs and generated indexes;
- document complexity and allocation behavior for non-trivial algorithms.

## Declarative-manifest direction

The current manifest already centralizes paths and runtime symbols, but signatures and ABI information still require coordinated maintenance.

The target pattern is one strongly typed declaration per operation that can generate or validate:

- canonical path and aliases;
- semantic type signature;
- native ABI signature;
- backend support;
- runtime symbol inventory;
- documentation export metadata;
- LSP catalog entries;
- parity tests.

This should be introduced incrementally. It must not hide important ownership or ABI behavior behind opaque code generation.

## Compatibility

Aliases exist only for documented compatibility or clear ergonomic value. An alias must not create a second canonical name.

Removing or renaming a public operation requires:

- compatibility classification;
- deprecation or migration policy;
- diagnostic/help update;
- changelog entry;
- versioning review;
- documentation and examples update.

## Quality requirements

Stdlib changes should validate:

- manifest uniqueness;
- signature and ABI completeness;
- runtime symbol availability;
- module discovery;
- native and JIT execution;
- ownership and cleanup for managed values;
- error behavior;
- C/debug support matrix;
- generated docs and LSP visibility;
- realistic example behavior.

Where an operation processes untrusted input, also inspect limits, allocation growth, path handling, encoding, timeout, and denial-of-service risks.

## Documentation requirements

Each public operation should document:

- purpose;
- parameters and return value;
- failure cases;
- ownership or resource lifecycle when relevant;
- mutation behavior;
- complexity for potentially expensive operations;
- platform differences;
- a minimal valid example;
- experimental status when applicable.

The sidecar docs should remain concise. Detailed design and maintenance rules belong in architecture, specification, or implementation documents.