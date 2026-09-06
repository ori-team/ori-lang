---
id: ADR-0005
title: Retire the C backend; retain the native reference pipeline
status: accepted
date: 2026-09-05
deciders: []
supersedes: []
superseded_by: []
related_docs:
  - docs/spec/14-backend-support.md
  - docs/product/requirements.md
related_code:
  - compiler/crates/ori-codegen/src/native_backend.rs
  - compiler/crates/ori-codegen/src/c_header.rs
  - compiler/crates/ori-driver/src/main.rs
  - compiler/crates/ori-types/src/stdlib.rs
---

# ADR-0005: Retire the C backend; retain the native reference pipeline

## Context

The partial C/debug route duplicated lowering, an inline runtime, Unicode
case-folding tables, and stdlib support metadata without being the semantic
reference. Native Cranelift AOT/JIT and the Rust runtime remain the reference.

## Decision

Retire C source emission in the current unreleased working tree, whose
workspace version remains **0.3.8**. This decision does not assign a future
release version or promise a release date or deprecation window.

- Remove `ori emit c`, `emit_c`, `run_emit_c`, and C source pipeline output.
- Remove `c_backend.rs`, `c_casefold.rs`, the inline C runtime, and the
  `c_backend_runtime` field / `c_backend` macro variant in the stdlib manifest.
- Keep Cranelift native AOT and JIT, runtime symbols, and native ABI contracts.
- Keep `c_header.rs` and `generate_c_header`: they generate FFI headers for
  native exports, not translated C program bodies. `extern c`, `@c_export`,
  and native library interoperability are not retired.
- Keep historical plans and release records as historical evidence.

## Considered options

1. Keep C as a debug route: preserves readable C and an independent execution
   route, but retains duplicate semantics and maintenance.
2. Retire C (selected): concentrates support and regression coverage on native
   execution, but removes C dumps and generated-C sanitizer workflows.
3. Extract a separate C crate: changes packaging without removing maintenance.

## Consequences and migration

Use `ori compile` for native artifacts and `ori run` for execution. Native
AOT/JIT differential tests cover their shared support surface. They are not
an independent C implementation and do not replace every generated-C check.
Scripts inspecting generated C or compiling it with GCC sanitizers need an
explicit redesign; changing their command to native compilation is not a
safe drop-in replacement. Native runtime sanitizer coverage is distinct from
instrumenting generated program code.

No replacement readable dump, portable backend, or sanitizer workflow is
promised by this decision. Existing native limitations remain documented in
[Spec 14](../../spec/14-backend-support.md).

## Self-hosting boundary

Self-hosting concerns the language used to implement the compiler. It does
not decide whether an Ori-written compiler uses Cranelift, another library,
or a custom object emitter. No custom object emitter, backend replacement,
bootstrap retirement, schedule, or release version is established here.
Any future backend decision requires its own evidence and acceptance criteria.

## Invariants

- Native execution remains the semantic reference.
- AOT and JIT must agree on their shared support surface.
- C ABI interoperability remains supported independently of C source emission.
- Removal does not change native runtime layouts or exported symbols.
