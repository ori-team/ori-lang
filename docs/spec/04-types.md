# Ori Language Specification — Chapter 04: Type System

> Status: normative
> Audience: compiler implementers, language designers
> Surface: **S3** (`0.3.0`)

---

## Overview

Ori is statically typed. Every binding, parameter, and return position has a
type known at compile time. There is **no global** (Hindley–Milner) type
inference for bindings.

**Local Nim-style inference (`0.3.1` + option B):** inside function bodies,
`const`/`var` may omit the type annotation when the right-hand side is obvious
on the same line:

```ori
const n = 1
const name = "Ada"
const u = User { name: "Ada", age: 36 }
const xs = [1, 2, 3]

-- Option B (accepted 2026-07-13): field / index / call / pipe with known type
const label = u.name
const first = xs[0]
const d = double(21)
const e = 21 |> double
```

If inference fails, the compiler emits `type.local_inference_failed` and asks
for an explicit annotation. Still **required** to write types on:

- `pub` items, parameters, and API return types;
- `try expr`, empty `[]`/`{}`, bare `none`, `void` results, and other
  non-obvious RHS;
- types inferred only from *later* uses in the block (no “context from use”).

**Not in scope:** Hindley–Milner global inference; block-wide solvers.

**Design decision (2026-07-01, restated):** global inference remains out of
scope permanently.

---

## Primitive Types

| Type | Description | Size |
|---|---|---|
| `bool` | Boolean: `true` or `false` | 1 byte |
| `int` | Signed 64-bit integer (default) | 8 bytes |
| `int8` | Signed 8-bit integer | 1 byte |
| `int16` | Signed 16-bit integer | 2 bytes |
| `int32` | Signed 32-bit integer | 4 bytes |
| `int64` | Alias for `int` | 8 bytes |
| `u8` | Unsigned 8-bit integer | 1 byte |
| `u16` | Unsigned 16-bit integer | 2 bytes |
| `u32` | Unsigned 32-bit integer | 4 bytes |
| `u64` | Unsigned 64-bit integer | 8 bytes |
| `float` | IEEE 754 64-bit float (default) | 8 bytes |
| `float32` | IEEE 754 32-bit float | 4 bytes |
| `float64` | Alias for `float` | 8 bytes |
| `string` | Immutable, valid UTF-8 text | Managed |
| `bytes` | Raw binary data | Managed |
| `void` | No useful value (return type only) | 0 bytes |

Primitive types are value types: they are copied on assignment.

`string` and `bytes` are immutable managed values with reference counting.
Assigning a `string` copies the reference, not the content.

---

## Compound Types

### Struct

A product type. All fields must be named.

```ori
struct Point
    x: int
    y: int
end

const p: Point = Point { x: 0, y: 0 }
```

Structs are value types. Assigning a struct copies all fields.
Fields that are managed types (`string`, `bytes`, collections) copy their
references.

**Field contracts** constrain the valid range of a field value:

```ori
struct Rectangle
    width: int  if it > 0
    height: int if it > 0
end
```

`it` is the contextual keyword that refers to the field value being validated.
Contracts are checked at construction time and on mutation. A violation is a
runtime panic (`contract.field_violation`).

### Enum

A sum type. Each variant is a named case.

```ori
enum Direction
    North
    South
    East
    West
end

enum Shape
    Circle(radius: float)
    Rectangle(width: float, height: float)
    Point
end
```

Variants may be:
- **Unit**: no payload (`North`)
- **Named variant**: all fields must have explicit names (`Circle(radius: float)`)

Positional (unnamed) enum payload is not allowed in Ori. All variant fields
must be named. This is required by the reading-first philosophy: `Circle(float)`
does not tell the reader what the float represents.

Enums are value types.

### Tuple

An ordered product of named positional values.

```ori
const pair: tuple[int, string] = tuple(1, "one")
```

Access by index:

```ori
const n: int    = pair.0
const s: string = pair.1
```

---

## Generic Collection Types

These types are built into the language and require no import.

| Type | Description |
|---|---|
| `list[T]` | Ordered, resizable sequence |
| `map[K, V]` | Key-value mapping. Current runtime supports `int`, `string`, and user-defined keys that implement `Hashable` and `Equatable` |
| `set[T]` | Unordered unique values. Current runtime supports `int`, `string`, and user-defined elements that implement `Hashable` and `Equatable` |
| `optional[T]` | A value that may be absent |
| `result[T, E]` | A value that represents success or failure |
| `range[int]` | An inclusive integer range |
| `lazy[T]` | Lazy value computed at most once through `lazy.once` and `lazy.force` |
| `any[Trait]` | Dynamic dispatch over a trait |

---

## Optional

`optional[T]` represents a value that may be absent. There is no `null`.

```ori
const name: optional[string] = some("Ada")
const empty: optional[string] = none
```

Constructors: `some(value)` and `none`.

Supported operations:

```ori
value.or(fallback)         -- unwrap or use fallback
value.or_return()          -- unwrap or propagate from enclosing function
try value                  -- unwrap or propagate from enclosing function
```

Current status: `.or(fallback)` is accepted for `optional[T]` and
`result[T, E]` in the checker, native backend, and C backend. The fallback is
evaluated only when the receiver is `none` or `error(_)`. `.or_return()` is
accepted as shorthand for propagation. The older `.or_return(expr)` form is
not implemented.

Pattern matching over `optional[T]`:

```ori
match maybe_name
case some(name):
    io.print(name)
case none:
    io.print("not found")
end
```

Binding shorthand:

```ori
if some(name) = maybe_name
    io.print(name)
end
```

---

## Result

`result[T, E]` represents an operation that may succeed or fail.

```ori
const ok: result[int, string]  = success(42)
const bad: result[int, string] = error("something went wrong")
```

Constructors: `success(value)` and `error(value)`.

Supported operations:

```ori
value.or(fallback)                   -- unwrap success or use fallback
value.or_return()                    -- unwrap success or propagate error
value.or_wrap("context message")    -- keep success, add context to error
try value                            -- unwrap success or propagate error
```

Current status: `.or(fallback)` and `.or_return()` are accepted. `.or_wrap(...)`
is accepted for `result[T, string]` and returns `success(v)` unchanged or
`error(context + ": " + e)` for `error(e)`. The context expression is evaluated
only on the error path. Use `try` or `match` when explicit error handling is
clearer. Postfix `expr?` was removed (S3); the compiler emits
`parse.question_propagate_removed`.

Pattern matching:

```ori
match load_config(path)
case success(config):
    use_config(config)
case error(msg):
    io.print(f"failed: {msg}")
end
```

---

## Range

`range[int]` is an inclusive integer range with a start and end value.

```ori
const r: range[int] = 0..9
```

The range `a..b` includes both `a` and `b`.
- If `a <= b`: ascending (0, 1, 2, ..., 9)
- If `a > b`: descending (9, 8, 7, ..., 0)
- If `a == b`: single element

Current v1 contract:

```ori
r.start       -- int: first value
r.end         -- int: last value
```

`length()` and `contains(...)` are not range methods in v1. Use `for` to
iterate a range, or compute membership with integer comparisons when needed.
Float ranges are not accepted by the current checker.

---

## Lazy

```ori
const expensive: lazy[int] = lazy.once(() => compute_heavy_value())
const value: int = lazy.force(expensive)
```

`lazy[T]` stores a zero-argument function that produces a `T`.

Rules:

- `lazy.once(() => value)` creates a lazy value.
- `lazy.force(expensive)` returns the computed `T`.
- The thunk runs at most once.
- Later `lazy.force` calls return the cached value.

This is useful when a value is expensive, optional in practice, or should be
computed only if another path needs it.

---

## Dynamic Dispatch (`any[Trait]`)

`any[Trait]` holds a value of any type that implements `Trait`, selected at runtime.

```ori
const shape: any[Drawable] = Circle(radius: 10.0)
shape.draw()
```

Rules:
- `any[Trait]` values have heap-allocated vtable dispatch.
- Prefer generics for performance-sensitive paths.
- `==` on `any[Trait]` is supported when the trait constraint includes equality (runtime vtable dispatch to the concrete type's `Equatable` implementation).
- Passing `any[Trait]` across FFI requires explicit ABI annotation.

---

## Func Types (Callable)

A function type describes the signature of a callable value:

```ori
const double: func(int) -> int = (x: int) => x * 2
var handler: func(string) -> bool
```

Syntax: `func(ParamType, ...) -> ReturnType`

A callable with no return value: `func(string)` (void return implied).

---

## Type Aliases

`alias` gives a name to an existing type. It does not create a new type.

```ori
alias UserMap  = map[int, User]
alias Callback = func(string) -> bool
```

Aliases are transparent: an alias and its target are interchangeable everywhere.

---

## Nominal Types (`newtype`)

`newtype` is the counterpart of `alias`: the same representation, but a
**distinct** type.

```ori
newtype UserId    = int
newtype AccountId = int
newtype Email     = string
```

| Form | Meaning |
|------|---------|
| `alias X = T` | another name for `T` — values flow freely |
| `newtype X = T` | a new type shaped like `T` — conversion is written out |

Rules:

- **Nominal.** `UserId` accepts neither an `int` nor an `AccountId`. This is
  the point: `transfer(from: AccountId, to: AccountId, by: UserId)` becomes a
  contract the compiler defends, not a comment.
- **Explicit both ways.** `UserId(7)` converts in; `int(id)` converts out
  (`string(mail)` for a newtype over `string`, and so on). There is no
  implicit conversion in either direction.
- **Zero cost.** The type is erased when lowering to HIR, so a `newtype` over
  `int` *is* an `int` at runtime — no wrapper struct, no allocation, and no
  trace in generated code.
- No type parameters yet: `newtype Pair[T] = …` is not accepted.

---

## `success()` — Void Result

When a function returns `result[void, E]`, `success()` with no arguments is valid:

```ori
ping() -> result[void, string]
    try send_packet()
    return success()
end

start() -> result[void, string]
    try ping()
    return success()
end
```

This is the exact analogue of `return` with no value in a `void` function.
The `void` value is implicit. `success()` with no args is a compile error
when the expected type is not `result[void, _]`.

---

## Equality (`==`)

Current implementation status:

- `==` and `!=` are implemented for numeric types, `bool`, `string`, `bytes`,
  `optional[T]`, `result[T, E]`, `tuple[...]`, `list[T]`, generic structs (with
  correct generic parameter substitution), opaque collections (`deque`, `queue`,
  `stack`, `linked_list`, etc.) when element types support equality, and
  non-generic structs whose fields also support equality.
- Function values are not comparable.
- `any[Trait]` values support structural equality via runtime vtable when the
  trait constraint permits it.
- Native and C/debug structural equality for `set[T]` and `map[K, V]` is
  implemented when keys/elements implement `Equatable` or builtin equality.

| Type | Current `==` behavior |
|---|---|
| numeric types | Value equality |
| `bool` | Value equality |
| `string` | UTF-8 text equality |
| `bytes` | Byte equality |
| `list[T]` | Structural equality when `T` supports equality |
| `map[K, V]` | Structural equality when `K` and `V` support equality |
| `set[T]` | Structural equality when `T` supports equality |
| `optional[T]` | Structural equality |
| `result[T, E]` | Structural equality |
| `tuple[...]` | Structural equality |
| non-generic `struct` | Structural equality when all fields support equality |
| generic `struct[T]` | Structural equality with generic substitution |
| opaque collections | Structural equality when elements/keys support equality |
| `any[Trait]` | Vtable equality when trait constraint allows |
| `func(...)` | Compile error |

Structural equality rules:

- Lists compare length and elements in order.
- Maps compare key-value pairs independent of insertion order.
- Sets compare elements independent of insertion order.
- Tuples and structs compare fields in declaration order.

**`Equatable` override:** use `apply T` / `use Equatable` for custom equality:

```ori
apply User
    use Equatable
        equals(other: User) -> bool
            return self.id == other.id
        end
    end
end
```

For user-defined types, `==` and `!=` use `equals()` when the type applies
`ori.core.Equatable`.

**Structs with incomparable fields:** if a struct contains a `func`,
`any[Trait]`, or another unsupported field, using `==` on that struct is a
compile error.

---

## Subtyping and Conversion

Ori does not have implicit type coercion. All conversions are explicit.

**Integer widening** is not implicit. Use the conversion functions:

```ori
const n: int  = 42
const b: u8   = u8(n)         -- explicit narrowing (runtime check)
const w: int64 = int64(n)     -- explicit widening
```

**Conversao para string:** o compilador aceita escalares built-in, `string`
e valores concretos definidos pelo usuario que implementam
`ori.core.Displayable`.

```ori
const s: string = string(42)
const t: string = string(3.14)
const b: string = string(true)
const same: string = string("ready")

struct Resource
    id: int
end

apply Resource
    use ori.core.Displayable
        display(self) -> string
        return "Resource#" + string(self.id)
    end
end

const r: Resource = Resource { id: 7 }
const label: string = string(r)
const line: string = f"value={r}"
```

**Type checking at runtime** (for `any[Trait]`):

```ori
if shape is Circle
    -- shape is accessible as Circle in this block
end
```

---

## Type Compatibility Rules

1. A `result[T, E]` is compatible with `result[T, F]` only if `E == F`.
2. A `list[T]` is compatible with `list[U]` only if `T == U` (no covariance).
3. Generic type parameters are invariant by default.
4. An `any[Trait]` accepts any concrete type implementing `Trait`.
5. A `func(T) -> R` is compatible with `func(T) -> R` only when signatures match exactly.
