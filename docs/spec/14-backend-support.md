# Backend support matrix

Status: current as of 2026-09-05 (workspace **0.3.8**, unreleased changes).

Native Cranelift AOT/JIT plus the packaged Rust runtime are the semantic
reference. C source emission and `ori emit c` have been removed under
[ADR-0005](../decisions/adr/0005-deprecate-and-retire-c-backend.md).
This does not remove C ABI interoperability: `c_header.rs`, generated native
export headers, `extern c`, and `@c_export` remain supported.

## Execution routes

| Route | Implementation | Contract |
|---|---|---|
| Native AOT | Cranelift object emission, native runtime, platform linker | `ori compile`, `ori build`, native executable/library artifacts |
| JIT | Cranelift JIT and staged runtime cdylib | Available to `ori run`; must agree with AOT on their shared support surface |
| C source emission | Removed | No CLI or codegen support; generated-C scripts require explicit migration |

The table below describes native feature support, not a guarantee that every
AOT artifact mode exists in JIT. Shared behavior is covered by
`compiler/crates/ori-driver/tests/differential_testing.rs`.

## Native feature summary

| Feature | Support | Notes |
|---|---|---|
| Basic expressions, statements, functions and imports | yes | Native tests cover local and transitive imports and entry modules. |
| Structured `@cfg` | yes | Shared frontend filtering precedes HIR, docs, and exported ABI. |
| Structs, enums and tuples | yes | Native ABI has layout tests. |
| Traits and `any[Trait]` | yes | Native dynamic dispatch. |
| Generics and monomorphization | yes | Generic functions and imported generic traits. |
| Lists, maps, sets, deques, queues, stacks | yes | Native runtime owns ARC edges. |
| Structural equality | partial | Primitives, `bytes`, `optional`, `result`, tuples, lists, generic structs, sets and maps when keys/elements support equality. |
| Hash tables, trees, graphs, heaps | yes | Native stdlib operations. |
| JSON | yes | Native runtime-backed `json.Value`, parsing and serialization. |
| `ori.net` (TCP/TLS/UDP) | yes | rustls and native I/O; async read/write/accept/UDP use the shared reactor; connect/TLS use worker futures. |
| File I/O async | yes | Worker futures and background Jobs. |
| `bytes` with internal NUL | partial | Native preserves embedded NUL bytes; `string` rejects internal NUL at conversion boundaries. |
| Unicode string operations | yes | Length, slices, indexing, search and iteration use scalar values, not byte offsets. Grapheme APIs are not implied. |
| Async functions and `await` | documented subset | See below. |
| `using` resource cleanup | yes | Sync and async disposal on normal return, propagation, cancellation, failure and `break`. |
| `core.Destructor` | yes | AOT/JIT invoke the callback before field cleanup. |
| `lazy.once` / `lazy.force` | yes | Inline native codegen. |
| LSP diagnostic positions | yes | UTF-16 columns and CRLF handling. |

## Native async subset

Covered by `compiler/crates/ori-driver/tests/concurrency_async.rs`:

- `await` inside `if`, `else`, `match`, `while`, `for`, and nested control flow.
- `await future` as a top-level expression statement.
- `const x: T = await future`, `return await future`, and `const x: T = try await future`.
- Awaited values in top-level return expressions, call arguments, operators,
  and statement conditions.
- Async `using` disposal on scope exit, cancellation, failure, `try`, and `break`.
- Multiple awaits with ARC locals preserved across suspension.

The promised subset is closed (LANG-1). Residual layout/planner failures are
not a promise of arbitrary async shapes. Unsupported shapes fail explicitly
with `backend.native_unsupported` rather than silently changing semantics.

## `backend.native_unsupported` inventory

| Code path | Classification |
|---|---|
| Async frame layout or state-machine planners cannot represent the body | Residual async; message identifies the function |
| Indexed assignment on unsupported managed bases | Non-async backend gap |
| `for` iterable or element without native iterator ABI | Non-async backend gap; covered by `compile_rejects_for_iterable_without_native_abi` |
| Unknown map/set/hash-table/tree/graph/heap runtime call | Internal defense; valid stdlib paths should resolve before emission |

## Stdlib maintenance

`compiler/crates/ori-types/src/stdlib.rs` owns runtime paths, aliases, symbols,
`native_runtime`, semantic signatures and native ABI metadata. There is no
`c_backend_runtime` field or `c_backend` macro variant. Compiler intrinsics
may use dedicated native lowering rather than a runtime symbol.
See [Spec 12](12-stdlib.md) and [Spec 15](15-stdlib-maintenance.md).

## Rules for future work

- Add positive native tests before expanding a supported row.
- Keep negative tests for intentionally blocked shapes.
- Verify AOT/JIT agreement on their shared support surface.
- Update this matrix with implementation changes; do not claim complete async
  support while a promised shape reaches `backend.native_unsupported`.
- Reopen LANG-RES with a concrete valid program and native blocker, not a
  removed C-backend limitation.
