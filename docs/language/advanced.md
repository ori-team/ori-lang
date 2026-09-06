# Advanced language features

> **Audience:** readers who finished the [language tour](tour.md)
> **Surface:** implemented Ori S3 plus the documented native extensions
> **Normative source:** [types](../spec/04-types.md), [functions](../spec/07-functions.md), [traits](../spec/08-traits.md), and [generics](../spec/11-generics.md)
> **Portuguese:** [advanced.pt-BR.md](advanced.pt-BR.md)

This page collects features that are stable enough to use but too specialized
for the first ten minutes of the tour. Each section states the current limit;
the limit is part of the contract, not an invitation to guess a future syntax.

## Arrays and slices

`list[T]` grows dynamically. `array[T, N]` (or `array[T, size: N]`) has its length
in the type and stores its elements inline:

```ori
struct Grid
    cells: array[int, 4]
    label: string
end

main()
    var values: array[int, 3] = [1, 2, 3]
    values[1] = 99
    const grid: Grid = Grid { cells: [10, 20, 30, 40], label: "grid" }
end
```

Rules:

- the length is part of the type; `array[int, 3]` and `array[int, 4]` do not substitute for one another;
- positional `array[int, 4]` and named `array[int, size: 4]` represent the same type;
- literals must have the exact length;
- out-of-range constant indexes are compile-time errors;
- the current native backend requires scalar or inline struct array elements.

`slice[T]` is a read-only O(1) window over a list. It keeps the owning list
alive and observes later writes to that list. `lists.slice` copies; a window
does not:

```ori
import ori.list as lists
import ori.slice as sl

var values: list[int] = [10, 20, 30]
const window: slice[int] = lists.window(values, 0, 2)
const length: int = sl.len(window)
```

See [04-types.md](../spec/04-types.md#slices-slicet) for the complete API and
the aliasing example.

## Lazy values and handles

`lazy[T]` stores a zero-argument computation and evaluates it at most once:

```ori
const delayed: lazy[int] = lazy.once(() => compute_value())
const value: int = lazy.force(delayed)
```

`handle[T]` is an opaque foreign/resource-shaped value. It is useful at an FFI
boundary, but it does not give Ori code a raw pointer or permission to free
memory manually, is not transferable across tasks, and is only borrowed for the
lifetime promised by the host API. Export aggregates containing borrowed
handles are rejected. `==` and `!=` compare pointer identity only and never
dereference the pointee; semantic equality, lifetime, and foreign-thread
affinity are not yet a complete language contract (`LANG-HANDLE-1`). To create
an explicit null sentinel, assign `handles.null()` to a `handle[T]` value. To
test only the sentinel, import `ori.handle` and call `handles.is_null(value)`;
this never dereferences the pointer. `@c_export` validates opaque managed handles against the runtime
registry before entering Ori code. For concrete non-generic payloads it also
checks the registered payload size and compiler source-type tag; null, foreign,
wrong-size, or wrong-type pointers follow the deterministic bounds-failure
path. The generated wrapper has no provenance-only fallback: a missing concrete
layout or source-type tag is a compile-time code-generation error. Hosts must
therefore use the handle from the matching export. The public safety rules are in
[16-runtime-ffi-safety.md](../spec/16-runtime-ffi-safety.md).

Generic graph nodes use the same `Equatable.equals` contract as maps and sets.
For a user-defined struct, the first concrete operation on a `graph.Graph[T]`
installs that callback; equivalent values then share one node and undirected
edge queries compare by value. `graph.clone` and `graph.transitive_closure`
preserve the callback. Enums support structural `==`/`!=` directly and can be
used as graph nodes when they carry `Hashable`; explicit `Equatable` still
overrides structural equality. Non-recursive structural keys use generated
native hash callbacks. A user-defined `hash(self) -> int` method on `Hashable`
overrides the generated callback. If explicit `Equatable` is non-structural
and no `hash` method is supplied, the runtime uses a constant-hash correctness
fallback; recursive admission and performance tuning remain open.

## Contracts and variadic parameters

A parameter contract checks a value when the function is called:

```ori
sqrt(value: float if it >= 0.0) -> float
clamp(v: int, lo: int, hi: int if it >= lo) -> int
```

The special name `it` means “the value of this parameter”. A violation is a
runtime diagnostic (`contract.param_violation`).

Only the final parameter may be variadic. Inside the function it is a list, and
the caller can spread an existing list with `..`:

```ori
public log(prefix: string, values: any[core.Displayable]...)

const parts: list[string] = ["a", "b", "c"]
concat(..parts)
```

## Associated types and functions

An associated type is scoped to one `use` section:

```ori
apply Bag use Container
    alias Item = string

    first_item(self) -> Item
        return self.label
    end
end
```

Traits without associated types use the compact direct header `apply Type: Trait`.

A method with `self` is an instance method. Inherent methods live directly in the
`struct` definition:

```ori
struct User
    name: string
    age: int

    make_empty() -> User
        return User { name: "", age: 0 }
    end
end
```

The old `type Item = ...` spelling is rejected; use `alias` inside `use`.

## Const generics

Const generics are compile-time values, not runtime fields:

```ori
struct Buffer[const size: int]
    used: int
end

const small: Buffer[size: 8] = Buffer { used: 0 }
```

CT-0 accepts side-effect-free integer and boolean expressions. A const
parameter must currently be passed directly; symbolic monomorphization such as
`size: capacity + 1` is outside the current contract.

## Nominal types

`alias` is transparent. `newtype` is nominal and zero-cost at runtime:

```ori
newtype UserId = int

make_id() -> UserId
    return UserId(7)
end

as_int(id: UserId) -> int
    return int(id)
end
```

Ori does not silently mix `UserId` and `int`; conversions must be written.

## Declaration attributes

Ori recognizes a closed set of top-level declaration attributes:
`@test`, `@deprecated("message")`, `@inline`, `@no_inline`, `@noalloc`,
`@align(N)`, `@cfg`, `@repr("C")`, and `@c_export`. Unknown attributes are errors.

- `@noalloc` statically verifies that the function performs no dynamic heap allocations (prohibits `list`/`map`/`set` literals, interpolated strings, closures, `await`, `using`, dynamic collection loops, and calls to functions not marked `@noalloc`).
- `@align(N)` explicitly sets struct alignment to a power-of-two (1, 2, 4, 8, 16, 32, 64), matching GPU uniform buffers and FFI boundaries (`alignas(N)`).

`@repr` is intentionally narrow: only exact `@repr("C")` on a struct is
accepted. It selects the supported C-compatible layout route. Packed layouts
and custom layout strings do not exist.

`@cfg` now selects top-level declarations before names and types are checked.
It uses structured predicates rather than free-form strings:

```ori
@cfg(all(target_family: unix, feature: tls))
public connect_securely()
end

@cfg(not(execution_profile: embedded))
public spawn_process()
end
```

The supported keys are `target_os`, `target_arch`, `target_family`,
`execution_profile`, and manifest-declared `feature`. Predicates compose with
`all`, `any`, and `not`. Syntax is still checked in inactive code, while name
and type errors inside it are not. See the [normative attribute rules](../spec/02-lexical.md#conditional-compilation)
and [manifest feature fields](../spec/17-project-and-docs.md#oriproj).

## Portable SIMD Vectors (`simd[T, N]`)

Ori provides first-class fixed-width SIMD vector types (`simd[float32, 4]`, `simd[int32, lanes: 4]`)
lowered directly to CPU vector registers (x86_64 SSE/AVX and ARM NEON). They support direct parallel
arithmetic operators (`+`, `-`, `*`, `/`) and lane indexing:

```ori
const a: simd[float32, 4] = [1.0f32, 2.0f32, 3.0f32, 4.0f32]
const b: simd[float32, 4] = [10.0f32, 20.0f32, 30.0f32, 40.0f32]
const c: simd[float32, 4] = a + b    -- single SIMD vector instruction
const x: float32 = c[0]             -- lane extraction
```

Supported combinations: `float32`/`int32` x 2, 4, 8, 16; `float64`/`int64` x 2, 4; `int16`/`u16` x 4, 8, 16; `int8`/`u8` x 8, 16.

## Scoped Memory Arenas (`mem.region`)

`mem.region()` creates a high-performance scoped bump-arena for frame-temporary allocations (60/120 FPS game
loops, visibility sets, rendering command queues), bypassing ARC reference counting:

```ori
import ori.mem as mem

main()
    using r: mem.Region = mem.region()
    -- perform frame allocations...
    mem.reset(r)           -- O(1) bulk reset
end                        -- deterministic cleanup via core.Disposable on scope exit
```

Escape analysis guarantees:
- A `Region` cannot escape its declaring block via `return` (`using.escape`).
- A `Region` cannot cross threads or tasks (it is not `Transferable`).

## Contiguous buffers (`buffer[T]`)

`buffer[T]` represents flat, contiguous heap memory for numeric arrays, pixel arrays, and audio samples.

```ori
import ori.buffer as buf

var pixels: buffer[int] = buf.alloc[int](1920 * 1080)
buf.set(pixels, 0, 0xFF0000FF)
```

## Custom destructors

Structs and enums can implement `core.Destructor` with `mut destroy(self) -> void` to free external resources deterministically before the ARC payload is deallocated:

```ori
struct ForeignResource
    handle: int
end

apply ForeignResource use core.Destructor
    mut destroy(self) -> void
        -- Close OS descriptor or foreign resource
    end
end
```

## What is intentionally not here

Higher-kinded types, explicit ownership-transfer syntax, direct collection ABI
layouts are not stable S3 features. Their current
status belongs in [BACKLOG.md](../planning/BACKLOG.md) and
[14-backend-support.md](../spec/14-backend-support.md), not in user examples.
