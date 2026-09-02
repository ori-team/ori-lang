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

All integer types support bitwise operations (`&`, `|`, `^`, `~`, `<<`, `>>`)
since 0.3.8 (GFX-BITWISE-1); see Chapter 05 for precedence and shift
semantics (arithmetic `>>` on signed, logical on unsigned).

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
| `array[T, size: N]` | Fixed-length sequence stored **inline** — see below |
| `buffer[T]` | Mutable, contiguous, fixed-length heap block — see below |
| `slice[T]` | A read-only **window** over a `list[T]` — see below |
| `map[K, V]` | Key-value mapping. `int` and `string` use native hashing; non-recursive user-defined struct or enum values with `Hashable` use a user `hash(self) -> int` method when present, otherwise generated structural hashing. Explicit, non-structural `Equatable` without `hash` uses a constant hash (`LANG-COLL-EQHASH-1`) |
| `set[T]` | Unordered unique values. `int` and `string` use native hashing; non-recursive user-defined struct or enum values with `Hashable` use a user `hash(self) -> int` method when present, otherwise generated structural hashing. Explicit, non-structural `Equatable` without `hash` uses a constant hash (`LANG-COLL-EQHASH-1`) |
| `graph.Graph[T]` | Directed or undirected graph. `int` and `string` nodes use native equality; user-defined structs and non-recursive enums with `Hashable` dispatch structural or explicit `Equatable` after the first concrete node operation and preserve that callback through graph copies. |
| `optional[T]` | A value that may be absent |
| `result[T, E]` | A value that represents success or failure |
| `range[int]` | An inclusive integer range |
| `lazy[T]` | Lazy value computed at most once through `lazy.once` and `lazy.force` |
| `any[Trait]` | Dynamic dispatch over a trait |

---

## Slices (`slice[T]`)

`slice[T]` is a read-only window over a `list[T]`. It holds the owning list plus
a range, never a copy of the elements, so taking one is O(1) whatever the
length.

```ori
import ori.list = lists
import ori.slice = sl

var xs: list[int] = [10, 20, 30, 40, 50]
const w: slice[int] = lists.window(xs, 1, 4)

const n: int = sl.len(w)        -- 3
const first: int = sl.get(w, 0) -- 20
```

The two ways of taking part of a list are **named differently on purpose**:

| Call | Result | Cost |
|---|---|---|
| `lists.slice(xs, a, b)` | a new `list[T]`, elements copied | O(n) |
| `lists.window(xs, a, b)` | a `slice[T]` over `xs` | O(1) |

Measured on a 100 000-element list: **2.4 ms** to copy, **12 µs** for a window.

### It is a window, and that is observable

```ori
const w: slice[int] = lists.window(xs, 1, 4)
lists.set(xs, 1, 999)
const v: int = sl.get(w, 0)     -- 999, not 20
```

This is the whole point, and it is why the window is a **distinct type** rather
than a faster `lists.slice`: sharing is visible in the source, at the call and
in the type. Changing `xs[1..3]` to share silently was rejected for the same
reason copy-on-write was (`docs/planning/adr-arc-cow-collections.md`).

### Safety

- **Read-only.** There is no `sl.set`. Writing through a window would make
  aliasing mutable, which is a much larger surprise than reading through one.
- **The owner cannot be freed underneath it.** Creating a window registers an
  ARC edge to the list, so a window may outlive the binding that made it.
- **The owner may be resized underneath it.** `lists.push` can move the element
  buffer, so the window stores the *list object* and resolves the buffer on each
  read. Reads past the owner's current length abort with a bounds message rather
  than reading freed memory.
- A window is not `Transferable`: it points at a list the sender owns, so it
  cannot cross a task or channel boundary.

---

## Borrowed handles (`handle[T]`)

`handle[T]` is currently a pointer-shaped, borrowed FFI value. It does not own
an ARC allocation, does not retain `T`, and is not `Transferable`; passing it to
`task.spawn` or a channel is rejected. A null handle is represented by the
runtime pointer value `0`. The safe probe `ori.handle.is_null(handle)` checks
that representation without dereferencing or retaining the pointee. The
generic `ori.handle.null()` constructor creates that null sentinel explicitly;
its type is inferred from the expected `handle[T]` type:

```ori
import ori.handle = handles

const missing: handle[int] = handles.null()

is_missing(value: handle[int]) -> bool
    return handles.is_null(value)
end
```

The constructor and probe do not establish pointee lifetime or thread affinity.
Ori still does not provide a safe dereference operation. A non-null handle must
come from a host/`extern c` API whose lifetime contract remains in force.

`==` and `!=` compare handle pointer identity only; they never dereference or
retain the pointee. Do not store a handle beyond the lifetime guaranteed by its
host API, or expose one in an aggregate return without a documented owner. The checker rejects `@c_export` aggregates containing a
borrowed handle, including nested structs and enum payloads. The complete
nullability, lifetime, equality, FFI, and cross-thread contract is tracked in
`LANG-HANDLE-1`; this section describes the current implementation boundary
rather than promising an ownership guarantee.

## Fixed-Size Arrays (`array[T, size: N]`)

`array` is the counterpart of `list`: the length is part of the **type**, so the
elements are stored inline — in the stack frame for a local, or inside the
owning struct for a field. No heap block, no length field, no reference
counting.

```ori
struct Grid
    cells: array[int, size: 4]
    label: string
end

main()
    var xs: array[int, size: 3] = [1, 2, 3]
    xs[1] = 99

    const g: Grid = Grid { cells: [10, 20, 30, 40], label: "grid" }
    const third: int = g.cells[2]
end
```

The length is written as a **named** const argument for the same reason
`Buffer[size: 8]` is (chapter 11): a bare `array[int, 4]` puts a loose number
between brackets, which reads as an index everywhere else in Ori. Any other name
is `parse.expected_array_size`.

The value may be a concrete CT-0 expression. CT-0 is deliberately smaller than
runtime Ori: integer literals, integer/boolean module constants, checked integer
arithmetic, comparisons, boolean logic, and inline `if` are accepted. It cannot
call functions, allocate, perform I/O, read the environment, or cross FFI.

```ori
const words: int = 4
const page_size: int = if words == 4 then words * 2 else 1

const page: array[int, size: page_size] = [1, 2, 3, 4, 5, 6, 7, 8]
```

### Rules

- **The length is part of the identity.** `array[int, size: 4]` and
  `array[int, size: 8]` are different types and do not substitute for each
  other.
- **A literal must match the length exactly** — `type.array_length_mismatch`
  otherwise.
- **A constant index is bounds-checked at compile time**
  (`type.array_index_out_of_bounds`). `list` cannot do this, because its length
  is not in the type.
- **`ori.mem.size_of` reports the whole block**: `size_of` of an
  `array[int, size: 4]` is 32, not the size of a pointer.
- The length may be a `const` parameter, which is what makes const generics
  useful. In CT-0 the parameter must be passed directly; symbolic arithmetic
  such as `size: cap + 1` is reserved for a later monomorphization extension:

  ```ori
  struct InlineString[const cap: int]
      data: array[u8, size: cap]
      len: int
  end
  ```

### Element types: the `Inline(T)` rule

Inline storage has no reference counting, so an element that needs ARC
ownership would be stored without a retain and released by nobody. The
element type must therefore be **inline**:

```text
Inline(bool)      = true
Inline(integer)   = true
Inline(float)     = true
Inline(array[T])  = Inline(T)
Inline(struct S)  = S has at least one field and every field of S is Inline
Inline(rest)      = false
```

A struct whose fields are all inline is itself inline, so graphics-friendly
types work directly:

```ori
struct Vec3
    x: float32
    y: float32
    z: float32
end

struct Triangle
    a: Vec3
    b: Vec3
    c: Vec3
end

const cube_verts: array[Vec3, size: 8] = [ … ]
```

Inline struct elements are stored contiguously with no heap block per element
and no ARC edge; `ori.mem.size_of` reports the whole block
(`array[Vec3, size: 8]` is 96 bytes on a 4-byte-aligned layout).

A struct holding any managed field (`string`, `list`, a managed struct, …) is
not inline; `array[SuchStruct, size: N]` is rejected with
`type.array_element_not_inline`, and the diagnostic names the offending field.
A recursive struct (`A { next: A }`) has no finite size and is likewise
rejected. Use `list[T]` when the elements are managed.

### Backend support

Native only. The C debug backend declines inline arrays rather than lowering
them to a heap list that would behave differently (chapter 14).

---

## Contiguous Buffers (`buffer[T]`)

`buffer[T]` is the numeric twin of `list[T]` for contiguous, **mutably
indexable** storage. The length is not part of the type; it is fixed at
allocation time and never grows — there is no implicit `reserve`, no hashing,
no versioned iterators, and no ARC edge per element.

```ori
var pixels: buffer[u32] = ori.buffer.new(width * height)
var depth: buffer[float32] = ori.buffer.new(width * height)
var verts: buffer[Vertex] = ori.buffer.new(vertex_count)

pixels[i] = 0xFF00FF00u32
const value: u32 = pixels[i]
const n: int = ori.buffer.len(pixels)
ori.buffer.fill(pixels, 0u32)
const span: slice[u32] = ori.buffer.as_slice(pixels)
```

### Rules

- **Elements must be inline.** Exactly the same rule as `array` (`Inline(T)`
  above): `buffer[string]`, `buffer[list[int]]`, etc. are rejected with
  `type.array_element_not_inline` (handled as `type.buffer_element_not_inline`
  once the diagnostic name is disambiguated). Empty structs and recursive
  structs are not inline.
- **Allocated with `ori.buffer.new(len)`.** `len` is an `int`; `len < 0` is
  rejected. Zero is valid.
- **Length-aware, indexed.** `buf[i]` is `T` (`i` may be `int`, bounds checked
  at compile time for constants and at runtime otherwise). `buf[i] = v`
  stores `v`. A length mismatch at construction is `type.buffer_length_mismatch`.
- **Bounds-checked assignment.** `buf[i] = v` is an `LValue::Index` whose
  index type is inferred as `int` (like `list[i] = v` and `array[i] = v`).
- **`ori.mem.size_of` reports a heap pointer** for `buffer` (the header, not the
  payload), consistent with `list`/`slice`.
- **Valid everywhere a collection is valid in `check_collection_runtime_limits`.**
- **Width-preserving `is_assignable_to`.** `buffer[int]` and `buffer[u32]` are
  different types; neither substitutes for the other.

### Backend support

Native only. The C backend declines `buffer` as `backend.buffer_unsupported`.



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
evaluated only when the receiver is `none` or `err(_)`. `.or_return()` is
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
const good: result[int, string] = ok(42)
const bad: result[int, string]  = err("something went wrong")
```

Constructors: `ok(value)` and `err(value)`. The pre-S3 spellings `success` /
`error` were removed and report `parse.result_ctor_renamed`.

Supported operations:

```ori
value.or(fallback)                   -- unwrap success or use fallback
value.or_return()                    -- unwrap success or propagate error
value.or_wrap("context message")    -- keep success, add context to error
try value                            -- unwrap success or propagate error
```

Current status: `.or(fallback)` and `.or_return()` are accepted. `.or_wrap(...)`
is accepted for `result[T, string]` and returns `ok(v)` unchanged or
`err(context + ": " + e)` for `err(e)`. The context expression is evaluated
only on the error path. Use `try` or `match` when explicit error handling is
clearer. Postfix `expr?` was removed (S3); the compiler emits
`parse.question_propagate_removed`.

Pattern matching:

```ori
match load_config(path)
case ok(config):
    use_config(config)
case err(msg):
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
- `==` on `any[Trait]` is supported through the runtime vtable. Equal
  concrete payloads compare structurally when their types provide equality;
  values with different concrete types compare unequal.
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

## `ok()` — Void Result

When a function returns `result[void, E]`, `ok()` with no arguments is valid:

```ori
ping() -> result[void, string]
    try send_packet()
    return ok()
end

start() -> result[void, string]
    try ping()
    return ok()
end
```

This is the exact analogue of `return` with no value in a `void` function.
The `void` value is implicit. `ok()` with no args is a compile error when the
expected type is not `result[void, _]`.

---

## Equality (`==`)

Current implementation status:

- `==` and `!=` are implemented for numeric types, `bool`, `string`, `bytes`,
  `optional[T]`, `result[T, E]`, `tuple[...]`, `list[T]`, generic structs (with
  correct generic parameter substitution), opaque collections (`deque`, `queue`,
  `stack`, `linked_list`, etc.) when element types support equality, and
  non-generic structs whose fields also support equality.
- Function values are not comparable.
- `any[Trait]` values support structural equality via their runtime vtable;
  the concrete payload type still determines whether a meaningful equality
  operation exists.
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
| `any[Trait]` | Vtable equality for compatible concrete payloads |
| `func(...)` | Compile error |

Structural equality rules:

- Lists compare length and elements in order.
- Maps compare key-value pairs independent of insertion order.
- Sets compare elements independent of insertion order.
- Tuples and structs compare fields in declaration order.

**`Equatable` override:** apply `core.Equatable` for custom equality. Import
the trait module first — a bare `use Equatable` reports `impl.trait_not_found`:

```ori
import ori.core = core

apply User use core.Equatable
    equals(self, other: User) -> bool
        return self.id == other.id
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

**String conversion:** `string(value)` accepts built-in scalars, `string`
itself, and concrete user-defined values that implement
`ori.core.Displayable`.

```ori
import ori.core = core

const s: string = string(42)
const t: string = string(3.14)
const b: string = string(true)
const same: string = string("ready")

struct Resource
    id: int
end

apply Resource use core.Displayable
    display(self) -> string
        return "Resource#" + string(self.id)
    end
end

const r: Resource = Resource { id: 7 }
const label: string = string(r)
const line: string = f"value={r}"
```

A struct **without** `Displayable` is not convertible: `string(value)` reports
`type.arg_type_mismatch`.

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
