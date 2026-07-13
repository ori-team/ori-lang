# Ori Language Specification — Chapter 10: Memory and Cleanup

> Status: normative
> Audience: compiler implementers

---

## Overview

Ori manages memory automatically. There are no explicit `malloc`/`free` calls.
Memory safety is guaranteed without requiring the programmer to track ownership
or lifetimes in the source language.

The memory model has two layers:
1. **Value semantics** — the default behavior for all types.
2. **Automatic Reference Counting (ARC)** — for managed types.

---

## Value Semantics

All types in Ori have value semantics by default.

```lua
const a: Point = Point(x: 1, y: 2)
const b: Point = a          -- b is a copy of a
```

Assigning a struct copies all its fields. There is no aliasing between `a`
and `b`. Mutating `b` (if `var`) does not affect `a`.

This applies to:
- All primitive types
- Structs
- Enums
- Tuples

---

## Managed Types

`string`, `bytes`, and all collection types (`list<T>`, `map<K, V>`, `set<T>`)
are **managed types**. They are heap-allocated and reference-counted.

Assigning a managed type copies the **reference**, not the heap data:

```lua
const a: list<int> = [1, 2, 3]
const b: list<int> = a          -- b holds the same reference as a
```

The heap data is freed when the last reference to it goes out of scope.

In practice, Ori's value semantics and immutability mean that managed types
behave as if they were copied — mutations produce new values, not in-place
changes — so the distinction is invisible in most code.

---

## Automatic Reference Counting (ARC)

The compiler inserts retain/release calls at compile time based on lexical scope.

Rules:
- A reference is retained when it is stored in a binding or passed to a function.
- A reference is released when the binding goes out of scope.
- In the Rust runtime used by the native backend, retain/release use atomic
  reference counts.
- The native backend links against the Rust `ori-runtime` static library. That
  runtime is the source of truth for managed values, ARC behavior, and runtime
  symbols used by `ori compile` and `ori test`.

**Cycle detection:** the native runtime tracks compiler-registered strong edges
between managed heap objects. `ori_arc_collect_cycles()` reclaims unreachable
cycles from that registered graph using a trial-deletion algorithm. The return
value is the number of heap objects reclaimed in the pass.

**Cooperative collection points:** the native backend calls
`ori_arc_collect_cycles()` at the following safe points:

- At the end of a sync function body (after scope cleanup, before returning
  control to the caller), when the function is a top-level scope
  (`managed_start == 0`).
- After dropping dead frame values following an `await` resume, if any values
  were released.
- Explicitly from Ori code via `ori.test.collect_cycles()` (intended for
  tests and diagnostic tooling).

The runtime does **not** currently run the collector on a periodic schedule or
after N allocations. Collection is driven by scope exits and explicit calls.

### Type-specific destructors

The native backend generates a Cranelift function destructor for each
composite type that has at least one managed field:

- **Structs:** `__dtor_struct_{id}` — loads each managed field by offset and
  calls `ori_arc_release` on it.
- **Enums:** `__dtor_enum_{id}` — switches on the active variant tag and
  releases the managed payload fields of that variant only.
- **Tuples:** `__dtor_tuple_{n}` — releases each managed element by offset.

When a managed struct/enum/tuple allocation is created via a literal, the
codegen registers the generated destructor as the `ori_alloc` destructor hook.
When the object's refcount drops to zero, the runtime calls that destructor
before freeing the header, which cascades releases to nested managed fields.

`optional` and `result` do not have dedicated destructors. They rely on
compiler-registered edges from the optional/result allocation to its managed
payload: when the optional/result is released, the runtime releases the owned
edges, which releases the payload.

### Leak check mode

The runtime exposes `ori.test.live_allocations()`, `ori.test.collect_cycles()`
and `ori.test.assert_no_leaks(label)` for test programs:

- `ori.test.live_allocations()` returns the number of live ARC-managed heap
  allocations (does not run the collector).
- `ori.test.collect_cycles()` runs the cycle collector and returns the number
  of objects reclaimed.
- `ori.test.assert_no_leaks(label)` runs the collector, then returns the
  remaining live count. If the `ORI_TEST_LEAK_CHECK=1` environment variable is
  set and the count is non-zero, it prints a diagnostic to stderr and aborts
  with a non-zero exit code so the test fails loudly.

These hooks are available on the native backend. The C debug backend provides
inline stubs that return 0 (the C backend has no ARC registry). See
`AGENTS.md` for the `ORI_TEST_LEAK_CHECK` env var convention.

### Backend Status

- The native backend inserts ARC retain/release calls for managed values.
- The Rust `ori-runtime` crate provides the runtime symbols consumed by the
  native backend.
- The standalone C backend remains a debug/transpile backend with partial
  feature parity. Its inline ARC runtime exists only for generated C output and
  does not define core language semantics.

---

## Async and ARC

Managed values that remain live across an `await` must stay retained until the
async continuation can use them again.

Current native status:

- `await` is accepted only inside `async func`.
- The runtime has pollable `future<T>` values, continuation registration, a FIFO
  executor queue, failed/cancelled internal states, and non-blocking timers.
- The current backend creates a `future<T>` immediately when an `async func` is
  called, allocates a native async frame, and schedules the generated `step`
  function on the native executor.
- Supported source-level `await` shapes suspend through `ori_future_poll` and
  `ori_future_on_ready`; they do not call `task.block_on`.
- Managed params, pre-await locals and await bindings stored in the frame have
  ARC edges. The state machine calculates liveness after each `await`, releases
  dead managed frame values after resumption, and still runs terminal cleanup.
- Failed/cancelled future states observed by the state machine are propagated by
  the generated async wrapper.
- `using` inside `async func` is **allowed**. The async frame stores the
  managed resource; `dispose()` is injected on scope exit. A residual compiler
  TODO remains for every terminal path (cancelled future, some `break`/`continue`
  combinations) — see master plan Etapa 4.

Async shapes outside the current state-machine subset are rejected before
Cranelift with `backend.native_unsupported` instead of falling back to a sync
bridge.

---

## `using` — Deterministic Cleanup

For resources that need explicit cleanup (file handles, network connections,
database connections), use `using`:

```lua
func read_file(path: string) -> result<string, string>
    using file: ori.fs.File = try ori.fs.open_read(path)
    const content: string = try ori.fs.read_all(file)
    return success(content)
end
```

When `file` goes out of scope, `file.dispose()` is called automatically.

### Cleanup Guarantee

`using` cleanup runs on **every** exit path from the scope:

- Normal `end` of block
- `return` statement
- `try`/`?` propagation (error path)
- `break` or `continue` in a loop
- Panic

### Cleanup Order (LIFO)

Multiple `using` bindings are disposed in **reverse declaration order**:

```lua
using a: ResourceA = try open_a()
using b: ResourceB = try open_b()
using c: ResourceC = try open_c()
-- When scope exits: c.dispose(), then b.dispose(), then a.dispose()
```

### `Disposable` Trait

A type participates in `using` by implementing `Disposable`:

```lua
trait Disposable
    mut func dispose()
end
```

Attempting to use a type in `using` that does not implement `Disposable`
is a compile error.

### Interaction with `try` and `?`

```lua
using conn: Connection = try get_connection()
-- If get_connection() returns error: conn is never bound, nothing to dispose.

const data: bytes = try conn.fetch(url)
-- If fetch() returns error: conn.dispose() IS called before error propagates.
```

---

## Stack vs Heap Allocation

The compiler decides allocation strategy. The programmer does not control this.

General rules (subject to optimization):
- Primitive types and small structs: stack-allocated.
- Managed types (`string`, `bytes`, collections): heap-allocated.
- `any<Trait>` values: heap-allocated (boxed).
- Closures that capture values: heap-allocated if they escape the current scope.

---

## No Manual Memory Management

Ori does not expose:
- Pointer arithmetic
- `malloc` / `free` / `realloc`
- Raw pointers (except through `extern c` FFI, where they must be handled carefully)
- Stack allocation directives

`ori.unsafe` provides escape hatches for low-level operations, but its use
is restricted to explicit `unsafe` annotated contexts and is not part of
the ordinary language surface.

---

## FFI Memory

When crossing the `extern c` boundary:
- Ori-managed values may not be passed as raw pointers without explicit conversion.
- C-allocated memory returned to Ori must be wrapped in a type implementing
  `Disposable` that frees it via the appropriate C function.
- The programmer is responsible for memory safety at FFI boundaries.

See the FFI documentation for detailed ABI shapes.

---

## `ori.mem`

`ori.mem` provides explicit memory inspection utilities:

```lua
import ori.mem as mem

mem.size_of(value)         -- size in bytes of value's static type
mem.align_of(value)        -- alignment in bytes of value's static type
```

These are compile-time constants. The argument is used as a type witness and
is not needed in ordinary code.

Current status: `ori.mem` is importable. The current parser does not support
type-argument call syntax such as `size_of<T>()`, so code should use the
expression-based form above.
