# Ori Language Specification — Chapter 11: Generics and Constraints

> Status: normative
> Audience: compiler implementers
> Surface: **S3** (`0.3.0`)

---

## Overview

Generics allow functions, structs, enums, and traits to be parameterized over
types. The compiler produces a specialized concrete implementation for each
distinct type argument (monomorphization).

---

## Generic Functions

```ori
identity[T](value: T) -> T
    return value
end

first[T](items: list[T]) -> optional[T]
    if len(items) == 0
        return none
    end
    return some(items[0])
end
```

Type parameters are declared in `[T]` after the name.
Multiple parameters: `[T, U]`, `[Key, Value]`.

A function declares its type parameters in **exactly one** of two ways, never
both:

| Form | Declares | Bounds |
|---|---|---|
| `name[T, U](params)` | `T`, `U` in the bracket list | none |
| `name for T: Trait (params)` | `T` in the bound clause | `T: Trait` |

Mixing them is a parse error — `name for T: Trait [T, U](params)` reports
`parse.unexpected_token` (`expected '(', found '['`). When a bound clause is
present, **every** type parameter must be named in it:

```ori
-- Correct: both parameters declared by the bound clause.
pick for K: Comparable, V: Displayable (k: K, v: V) -> K
    return k
end
```

Naming a type parameter the bound clause does not declare is
`type.undefined_name`.

---

## Generic Structs

```ori
struct Pair[A, B]
    first: A
    second: B
end

const p: Pair[int, string] = Pair { first: 1, second: "one" }
```

---

## Generic Enums

```ori
enum Either[Left, Right]
    Left(value: Left)
    Right(value: Right)
end
```

---

## Generic Traits

A trait may be declared with type parameters:

```ori
trait Container[Item]
    mut add(item: Item)
    get(index: int) -> optional[Item]
    length() -> int
end
```

> **Declaration-only today.** No `apply` block can implement a generic trait —
> `use Trait[Arg]` does not parse, and bare `use Trait` leaves the parameter
> unbound. See "Not supported" at the end of this chapter.

---

## Type Constraints (`for T: Trait`)

Type parameters may be constrained to require specific trait implementations:

```ori
max for T: Comparable (a: T, b: T) -> T
    if a.compare(b) > 0
        return a
    end
    return b
end
```

### Multiple Constraints

Constraints are separated by commas. Each entry names a type parameter and the
trait it must satisfy; the same parameter may appear more than once:

```ori
sorted_keys for K: Hashable, K: Comparable, V: Displayable (m: map[K, V]) -> list[K]
    -- ...
end
```

There is no separate `[K, V]` list — see the table above.

### Inline Value Contracts on Parameters

Value contracts on individual parameters use `if` after the type or after a
default value:

```ori
sqrt(value: float if it >= 0.0) -> float
```

This is a value contract (checked at runtime), not a type constraint.

### Negative Constraints

```ori
raw_copy for T: not Disposable (src: T, dst: T)
```

Prevents the function from being called with managed/resource types. Violations
report `generic.constraint_not_satisfied`.

---

## Type Inference in Generic Calls

Ori infers type arguments at call sites when they can be determined from the
argument types:

```ori
-- Type argument T inferred as int from the argument 42:
const result: int = identity(42)

-- Type argument T inferred as string from the list contents:
const name: optional[string] = first(["Ada", "Bo"])
```

When inference is ambiguous or impossible, the type argument must be explicit:

```ori
const empty: optional[int] = first[int]([])
```

---

## Monomorphization

The compiler generates a concrete function or type for each unique combination
of type arguments used in the program.

Think of a generic declaration as a mold. Each concrete type used with that mold
gets its own generated implementation.

```ori
identity(42)          -- may generate identity_int
identity("hello")     -- may generate identity_string
first([1, 2, 3])      -- may generate first_list_int
```

This means:
- Generic code has zero runtime overhead compared to hand-written typed code.
- The backend can optimize each concrete type separately.
- Large programs with many generic instantiations may have larger binaries
  because each concrete combination can produce another copy of the code.
- Compile time may increase when a generic API is used with many types.
- Circular generic instantiations are a compile error.

Example:

```ori
wrap[T](value: T) -> optional[T]
    return some(value)
end

const a: optional[int] = wrap(1)
const b: optional[string] = wrap("ori")
```

The compiler can lower this as if the program had two concrete functions:

```text
wrap_int(value: int) -> optional[int]
wrap_string(value: string) -> optional[string]
```

### Future direction

Monomorphization remains the default strategy for v1 because it is fast at
runtime and simple for native code generation.

Future work should reduce binary-size surprises without making normal code more
complex:

- report generic instantiation counts in `ori summary`;
- add compiler warnings for very large instantiation sets;
- deduplicate identical generated code when it is safe;
- study optional type erasure through `any[Trait]` for cold APIs, plugin
  boundaries, and package boundaries;
- keep monomorphization for hot paths and small programs.

---

## Supported Generic Combinations

Not all combinations of types and generic functions are supported. The compiler
reports a clear error when a type argument fails to satisfy a `for T: Trait` bound:

```
error[generic.constraint_not_satisfied]: T does not satisfy constraint
  --> src/app/main.orl:12:5
   |
12 |    const keys: list[K] = sorted_keys(my_map)
   |                          ^^^^^^^^^^^^^^^^
   |
   = why: K = User, but User does not satisfy Comparable
   = action: add `apply User` / `use Comparable` with `compare(other: User) -> int`
```

---

## `Self` in Generic Contexts

`Self` inside a `trait` or `apply` block refers to the implementing type.
It may be used as a type argument:

```ori
trait Cloneable
    clone() -> Self
end

apply Config use Cloneable
    clone(self) -> Config
        return Config {
            timeout: self.timeout,
            retries: self.retries
        }
    end
end
```

The compact header is required here: the block holds one trait and nothing else
(chapter 08). `self` is an explicit first parameter on the implementing method.

---

## Generic Type Aliases

```ori
alias IntMap[V]   = map[int, V]
alias Callback[T] = func(T) -> bool
```

---

## Limitations in v1

### Associated types

Ori's associated types are **implementation-side type aliases scoped to a `use`
section**. They are a naming tool for the implementer, not an abstract member
declared by the trait:

```ori
trait Container
    first_item(self) -> string
    size(self) -> int
end

apply Bag use Container
    alias Item = string
    alias Count = int

    first_item(self) -> Item
        return self.label
    end

    size(self) -> Count
        return self.count
    end
end
```

Rules:

- Declared with `alias Name = Type` **inside** a `use` section. The trait itself
  declares concrete signatures and knows nothing about the alias.
- Transparent: `Item` *is* `string`. An alias that resolves to a type the trait
  signature does not accept is `impl.wrong_signature` — the alias cannot widen
  the contract, only rename it.
- Scoped to its own `use` section. The name is undefined in a sibling `use`, in
  free members of the same `apply`, and outside the block (`type.undefined_name`).
- The pre-S3 spelling `type Name = …` is rejected with
  `parse.associated_type_keyword_removed`. `ori migrate-syntax` rewrites it.

> **Not** the Rust/Swift feature of the same name. A trait cannot declare an
> abstract associated type that each implementer fills in with a different
> concrete type; the trait signature is fixed. See "Not supported" below.

### Const generics

A struct may take a compile-time integer constant as a type parameter.
Declaration marks it with `const`; use sites **name** the argument:

```ori
struct Buffer[const size: int]
    used: int
end

struct Matrix[const rows: int, const cols: int]
    label: string
end

const b: Buffer[size: 8] = Buffer { used: 0 }
const m: Matrix[rows: 2, cols: 3] = Matrix { label: "2x3" }
```

Rules:

- **Arguments are named**, not positional. A bare `Buffer[8]` would read
  exactly like the index expression `frutas[8]`; `Buffer[size: 8]` reads as
  an argument, matching how calls, struct literals and enum payloads already
  name their values. A non-integer value is `parse.expected_const_arg_value`.
- The value is part of the type's identity: `Buffer[size: 8]` and
  `Buffer[size: 16]` are **different types** and do not substitute for each
  other.
- A struct literal takes the const arguments from the type it is checked
  against (`const b: Buffer[size: 8] = Buffer { used: 0 }`), since the value
  appears only in the annotation. Type arguments are not inherited this way —
  those come from the fields.
- The constant is a compile-time tag only: it occupies no storage and never
  reaches runtime. (Fixed-size arrays, which would consume it, do not exist
  yet.)

### Not supported

#### Higher-kinded types (HKT) — out of scope

A type parameter that ranges over **type constructors** (`F` standing for
`list`, `optional`, … so that `F[A]` is a type) is **not** a planned Ori
feature. The declaration syntax `trait Functor[F[_]]` currently parses, but no
implementation can satisfy it: the checker compares `F[int]` against the
implementer's `Wrapper[int]` literally and reports `impl.wrong_signature`.
Treat the parse acceptance as a wart, not a contract.

This is a deliberate rejection, not a missing milestone:

- A signature like `map[A, B](self: F[A], f: func(A) -> B) -> F[B]` holds three
  simultaneous abstractions, none concrete. That is the opposite of the
  reading-first goal in [`00-manifesto.md`](00-manifesto.md).
- Rust, Go, and Zig all decline full HKT; Swift covers the practical cases with
  associated types instead.
- Implementing it requires higher-order unification (substituting a
  *constructor* for a parameter), a substantially larger type-system change than
  anything else in this chapter, benefiting library authors rather than
  application readers.

Use a concrete trait per container, or the alias mechanism above.

#### Generic traits are declaration-only

A trait may be *declared* with type parameters (`trait Container[Item]`), but it
**cannot be applied**:

- `apply IntStack use Stackable[int]` — `parse.expected_identifier`; `use`
  accepts a bare trait name, with no type arguments.
- `apply IntStack use Stackable` with concrete method types —
  `impl.wrong_signature`, because the trait parameter is never bound.

Until `use Trait[Arg]` is implemented, declare one concrete trait per element
type.

#### Variadic type parameters

`tuple[T...]` is not supported (`parse.unexpected_token`); use `tuple[A, B]`
with fixed arity.

### Sanity tests

The forms above are verified in
`compiler/crates/ori-driver/tests/ori_spec.rs`:

| Test | Covers |
|---|---|
| `generic_accepts_generic_struct` | `struct Pair[A, B]` |
| `generic_accepts_type_inference` | type argument inferred at the call site |
| `generic_accepts_where_constraint` | `for T: Comparable` |
| `generic_rejects_constraint_not_satisfied` | bound violation is an error |
| `generic_accepts_negative_constraint` | `for T: not Disposable` |
| `generic_rejects_negative_constraint_violated` | negative bound violation |
| `generic_generic_struct_types_are_distinct` | `Pair[int, int]` ≠ `Pair[int, string]` |
| `compile_runs_associated_types_in_apply_use` | `alias Item = string` end-to-end |
| `check_rejects_associated_type_that_breaks_the_trait_signature` | alias cannot widen a contract |
| `check_keeps_associated_types_scoped_to_their_use_section` | alias scoping |
| `compile_runs_const_generics_with_named_arguments` | `Buffer[size: 8]` end-to-end |
| `check_treats_different_const_arguments_as_different_types` | const args are part of identity |
| `check_rejects_a_non_integer_const_argument` | non-integer const argument |

> Two earlier tests, `generic_accepts_hkt` and
> `generic_accepts_associated_type_in_trait`, were **removed**: both asserted
> only that a *declaration* parsed, so they passed while neither feature could
> be implemented. A test in this chapter must exercise an implementation, not
> just a declaration.
