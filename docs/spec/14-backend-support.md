# Backend support matrix

Status: current as of 2026-08-08 (FREEZE-1 / workspace 0.3.8-dev).
Residual cleanup: [`../planning/qa/residual-cleanup-2026-07-13.md`](../planning/qa/residual-cleanup-2026-07-13.md) · audit `tools/qa/residual_audit.sh`.

This page separates three things:

- Language promise: the feature is part of Ori.
- Native backend: Cranelift plus packaged Rust runtime.
- C/debug backend: C source emission used for debug and compatibility checks.

Legend:

- yes: supported and covered by tests.
- partial: supported only for a documented subset.
- no: intentionally rejected today.
- internal: only an internal defensive error path.

## Summary

| Feature | Language promise | Native backend | C/debug backend | Notes |
| --- | --- | --- | --- | --- |
| Basic expressions and statements | yes | yes | partial | Native is the main execution path. C/debug is not full parity. |
| Functions and imports | yes | yes | partial | Native tests cover local imports, transitive imports and entry module. |
| Structs, enums and tuples | yes | yes | partial | Native ABI has layout tests. |
| Traits and `any[Trait]` | yes | yes | partial | Native tests cover dynamic dispatch. |
| Generics and monomorphization | yes | yes | partial | Native tests cover generic functions and imported generic traits. |
| Lists, maps, sets, deques, queues, stacks | yes | yes | partial | Native runtime owns ARC edges. |
| Structural equality | yes | partial | partial | Native covers primitives, `bytes`, `optional`, `result`, tuples, lists, generic structs, `set[T]`, and `map[K,V]` when keys/elements support equality. C/debug covers the corresponding supported shapes but rejects `bytes` equality because its debug representation has no length-carrying payload. |
| Hash tables, trees, graphs, heaps | yes | yes | partial | Native tests cover stdlib operations. |
| JSON (`json.parse` / `json.Value`) | yes | yes | partial | C backend emits `ori_json_parse` FFI stubs without dedicated C lowering; execution requires native runtime. |
| `ori.net` (TCP/TLS/UDP) | yes | yes | no | Native runtime only (rustls). Sync path blocking; async uses shared I/O reactor with `poll(2)` readiness (STDLIB-4k) for read/write/accept/UDP; connect/TLS still worker+future (STDLIB-4b). |
| File I/O async | yes | yes | no | L1 `fs.read_text_async` / `write_text_async` (worker + future); L2 `read_text_in_background` Jobs. |
| `bytes` with internal NUL | yes | partial | partial | Native preserves embedded NUL bytes; the C/debug backend does not expose the length-carrying `bytes` representation. `string` still rejects internal NUL at conversion boundary. |
| Unicode `string.len`, `slice`, `index_of` | yes | yes | partial | Indices are Unicode scalar indices, not byte offsets. |
| Async functions and `await` | yes | yes* | no | *Promised native subset closed (LANG-1). Rare residual layout failures only — see inventory. C/debug rejects async. |
| `using` resource cleanup | yes | yes | partial | Sync and async `using` supported; async dispose on normal return, `try`, cancel, fail, and `break`. |
| `core.Destructor` automatic cleanup | yes | yes | no | Native AOT/JIT run the callback before field cleanup. C/debug rejects it with `backend.c_unsupported` rather than changing semantics. |
| `lazy.once` / `lazy.force` | yes | yes | partial | Native uses inline Cranelift codegen; C backend has dedicated lowering. |
| LSP diagnostics positions | yes | yes | n/a | LSP uses UTF-16 columns and handles CRLF. |

## Native async subset

Supported today (covered by `concurrency_async.rs`):

- `await` inside `if`, `else`, `match`, `while`, `for`, and other control-flow bodies (branching state machine).
- Nested loop bodies with `await`, including `for { while { await ... } }`.
- `await future` as a top-level expression statement.
- `const x: T = await future`.
- `return await future`.
- `const x: T = try await future`.
- `await` inside top-level return expressions, call arguments, and operators.
- `await` inside top-level statement conditions, such as `if await flag()`.
- `using` inside an `async` function with `dispose()` on scope exit, cancellation, failure, propagation (`try`), and `break`.
- Multiple awaits in the same async function with preserved ARC locals across suspensions.

### LANG-1 status (2026-07-13)

The **promised** native async subset above is **closed**: positive coverage lives in
`compiler/crates/ori-driver/tests/concurrency_async.rs` (loops, branches, match,
`using`, managed values across suspension, `try`, call/operator/condition
awaits, nested bodies).

Shapes that still emit `backend.native_unsupported` are **not** open async
promises — they are either permanent v1 exclusions or non-async backend gaps:

| Kind | What | v1 policy |
| --- | --- | --- |
| Residual async | Async body whose params/locals cannot be laid out for the frame (`cl_type` fails) or both state-machine planners reject the body | **Documented residual** — rare; fails with an actionable message naming the function |
| Non-async | `for` over types without iterator ABI | **Permanent until** an iterable is given a native next-ABI |
| Non-async | Indexed assignment on unsupported managed bases | **Permanent until** that base gains a store path |
| Internal defense | Unknown map/set/graph/… runtime call name | Should not surface from valid stdlib use |

Current failure mode:

- Native codegen emits `backend.native_unsupported` with a direct message when a shape is outside the supported subset.
- C/debug continues to reject all async (see **LANG-3** / C async section).

## `backend.native_unsupported` inventory

| Message / code path | Classification | Tests |
| --- | --- | --- |
| Async function contains an `await` shape not covered by the state machine | residual async (not a promised gap) | Message path in `emit_async_wrapper`; positive suite covers promised subset |
| Indexed assignment base unsupported | backend gap (non-async) | Negative / defensive |
| `` `for` iterable type `{ty}` `` | backend gap (non-async) | Positive: supported iterators; negative: `compile_rejects_for_iterable_without_native_abi` |
| `` `for` element type `{ty}` `` | backend gap (non-async) | Same |
| `` map runtime call `{name}` `` | internal defense | Stdlib paths should resolve before emission |
| `` hash_table runtime call `{name}` `` | internal defense | Same |
| `` graph runtime call `{name}` `` | internal defense | Same |
| `` set runtime call `{name}` `` | internal defense | Same |
| `` tree runtime call `{name}` `` | internal defense | Same |
| `` heap runtime call `{name}` `` | internal defense | Same |

## C/debug backend scope

Intentionally **not** supported on the C route:

- `async` functions, `await`, `task.*`, `channel.*`, `atomic.*`
- `json.parse` / structured `json.Value` (C emits FFI stubs only; no dedicated C lowering)
- `ori.net.*` (TCP/TLS/UDP; native runtime only)

C/debug **does** support (see `multifile_imports.rs` `build_c_backend_*`):

- Structural equality (structs, lists, maps, sets)
- `lazy.once` / `lazy.force` lowering
- Stdlib surfaces: math, format, os, time, random, mem, iter (partial), test asserts

## C/debug backend stdlib matrix (`c_backend` flag)

The `stdlib!` macro in `compiler/crates/ori-types/src/stdlib.rs` tags each
runtime function with a `c_backend` flag. When the flag is set, the C/debug
backend ships a matching implementation in its inline runtime header
(`ORI_RUNTIME_H`, enforced by the `c_backend_inline_runtime_exports_manifest_symbols`
test). Functions without the flag are native-runtime-only: the C backend may
still emit them as `extern` calls or lower them via dedicated code paths
(structural equality, string concat, lazy), but they do not have a C runtime
body and require the native Rust runtime to actually execute.

Legend:

- **yes**: every function in the module carries the `c_backend` flag.
- **partial**: a subset of the module carries the flag (see Notes).
- **no**: no function carries the flag; C backend emits extern stubs or dedicated lowerings only.
- **inline**: handled by inline C codegen, not runtime FFI (no flag needed).

| Module | `c_backend` flag | C execution | Notes |
| --- | --- | --- | --- |
| `io.print`, `io.println` | yes | yes | Flagged. |
| `io.eprint`, `io.eprintln`, `io.read_line` | yes | yes | **LANG-2:** real C bodies (stderr + getline/fgets). |
| `math.*` | yes | yes | All 16 functions flagged. |
| `time.now`, `time.sleep`, `time.duration_ms` | yes | yes | All flagged. |
| `format.*` | yes | yes | number, percent, hex, binary, date, datetime, bytes_size. |
| `os.*` | yes | yes | args, env, exit, pid, platform, arch. |
| `random.*` | yes | yes | int, float, bool, choice, shuffle. |
| `iter.*` | yes | yes | map, filter, any, all, count_where, take, skip, reverse, reduce, find, sort, sort_by, unique, flat_map, zip, partition, group_by, flatten. |
| `test.assert`, `test.assert_eq`, `test.assert_ne`, `test.fail` | yes | yes | Flagged. |
| `test.live_allocations`, `test.collect_cycles`, `test.assert_no_leaks` | no | extern only | Leak checks require native ARC runtime. |
| `string` (global), `int`, `float` builtins | yes | yes | Conversion builtins flagged. |
| `len` (global builtin) | yes | yes | **LANG-2:** maps to string length in C. |
| `string.*` | yes | yes | **LANG-2:** len/concat/slice/contains/starts/ends/trim/case/replace/split/chars/join/index_of/repeat/pad. |
| `bytes.*` | no | extern only | Native runtime (opaque payload ABI). |
| `convert.*` | yes | yes | **LANG-2:** float/bool/string conversions in C. |
| `list.*`, `deque.*`, `queue.*`, `stack.*` | no | extern only | Native runtime owns ARC edges. |
| `linked_list.*`, `doubly_linked_list.*` | no | extern only | Native runtime. |
| `tree.*` | no | extern only | Native runtime. |
| `map.*`, `set.*`, `hash_table.*` | no | dedicated lowering + extern | C backend lowers structural equality and iterator ABI; ops are extern. |
| `graph.*`, `heap.*` | no | extern only | Native runtime. |
| `json.parse`, `json.stringify`, `json.stringify_pretty` | no | extern stub | C emits FFI stub without dedicated lowering; execution requires native runtime. |
| `fs.*`, `files.*` | no | extern only | Native runtime. |
| `net.*` | no | extern only / rejected | TCP/TLS/UDP; rustls + blocking I/O in native runtime only. |
| `task.*`, `channel.*`, `atomic.*` | no | rejected | C backend rejects async/concurrency symbols entirely. |
| `lazy.once`, `lazy.force` | inline | yes | Inline C codegen; no runtime FFI flag. |
| `panic` | no | extern only | Native runtime. |

### Rules for the `c_backend` flag

- Adding a new stdlib function with a C runtime body: use the `c_backend`
  variant of `stdlib!` and add the matching body to `ORI_RUNTIME_H`. The
  `c_backend_inline_runtime_exports_manifest_symbols` test enforces consistency.
- Adding a native-only function: omit the flag. Document the native-only
  constraint in this matrix.
- Changing a row from `no` to `yes` requires a positive `build_c_backend_*`
  test in `multifile_imports.rs`.

## C/debug async and concurrency scope

Full async/concurrency parity in the C/debug backend is **intentionally not
part of its contract**.
The native Cranelift backend is the reference implementation for `async` functions,
`await`, `task.*`, `channel.*`, and `atomic.*`.

Current C/debug behaviour:

- `async` functions / `await` in user code: rejected at C codegen with an actionable
  message (`backend.c_unsupported` via `ori emit c`).
- Stdlib async/concurrency symbols: rejected at C codegen (same route).
- Sync subset (`ori emit c` on non-async programs): supported per the matrix above.

Rationale: async on native uses a dedicated state machine, ARC frame edges, and
runtime executor hooks that would duplicate a large fraction of `ori-runtime`
in `ORI_RUNTIME_H`. The C route remains a **debug/transpile** path for sync
programs, not a second production backend. Maintenance covers invalid C,
compiler crashes, and wrong semantics inside the documented synchronous
subset. New language features may reject C emission with
`backend.c_unsupported`.

Do not mark C async/concurrency as partial or supported unless a future
language decision explicitly promotes C to a product backend.

## Rules for future work

- Add a positive native test before changing a row from partial to yes.
- Keep a negative test when a shape is intentionally blocked.
- Update this matrix in the same commit as the implementation change.
- Do not call async "complete" while any **promised** `await` shape still reaches
  `backend.native_unsupported`. LANG-1 (2026-07-13): promised native subset is
  covered; remaining codes are residual/layout or non-async gaps.
- **LANG-RES (2026-07-13):** closed for product surface — no known
  product-blocking native residual. Gate test:
  `compile_runs_lang_res_product_surface_native`. Closure write-up:
  `docs/planning/historico/lang-res-closure.md`. Reopen only with a concrete blocker
  program (valid language surface + `backend.native_unsupported`).
