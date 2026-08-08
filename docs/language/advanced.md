# Advanced language features

> **Audience:** readers who finished the [language tour](tour.md)
> **Surface:** implemented Ori S3 plus the documented native extensions
> **Normative source:** [types](../spec/04-types.md), [functions](../spec/07-functions.md), [traits](../spec/08-traits.md), and [generics](../spec/11-generics.md)
> **Portuguese:** [advanced.pt-BR.md](advanced.pt-BR.md)

This page collects features that are stable enough to use but too specialized
for the first ten minutes of the tour. Each section states the current limit;
the limit is part of the contract, not an invitation to guess a future syntax.

## Arrays and slices

`list[T]` grows dynamically. `array[T, size: N]` has its length in the type and
stores its elements inline. The size argument is named because a bare number in
`[]` looks like an index everywhere else:

```ori
struct Grid
    cells: array[int, size: 4]
    label: string
end

main()
    var values: array[int, size: 3] = [1, 2, 3]
    values[1] = 99
    const grid: Grid = Grid { cells: [10, 20, 30, 40], label: "grid" }
end
```

Rules:

- the length is part of the type; `array[int, size: 3]` and `array[int, size: 4]` do not substitute for one another;
- literals must have the exact length;
- out-of-range constant indexes are compile-time errors;
- the current native backend requires scalar array elements;
- the C/debug backend does not provide full array parity.

`slice[T]` is a read-only O(1) window over a list. It keeps the owning list
alive and observes later writes to that list. `lists.slice` copies; a window
does not:

```ori
import ori.list = lists
import ori.slice = sl

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
memory manually. The public safety rules are in
[16-runtime-ffi-safety.md](../spec/16-runtime-ffi-safety.md).

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

A method with `self` is an instance method. Without `self`, it is an associated
function called through the type:

```ori
apply User
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

## What is intentionally not here

Higher-kinded types, explicit ownership-transfer syntax, direct collection ABI
layouts, and full C-backend parity are not stable S3 features. Their current
status belongs in [BACKLOG.md](../planning/BACKLOG.md) and
[14-backend-support.md](../spec/14-backend-support.md), not in user examples.
