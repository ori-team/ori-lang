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

Type parameters are declared in `[T]` after the name (or via `for T: Trait` bounds).
Multiple parameters: `[T, U]`, `[Key, Value]`.

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

```ori
trait Container[Item]
    mut add(item: Item)
    get(index: int) -> optional[Item]
    length() -> int
end
```

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

```ori
sorted_keys for K: Hashable, K: Comparable [K, V](m: map[K, V]) -> list[K]
    -- ...
end
```

### Inline Value Contracts on Parameters

Value contracts on individual parameters use `if` after the type or after a
default value:

```ori
sqrt(value: float if it >= 0.0) -> float
```

This is a value contract (checked at runtime), not a type constraint.

### Negative Constraints

```ori
raw_copy for T: not Disposable [T](src: T, dst: T)
```

Prevents the function from being called with managed/resource types.

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

apply Config
    use Cloneable
        clone() -> Config
            return Config {
                timeout: self.timeout,
                retries: self.retries
            }
        end
    end
end
```

---

## Generic Type Aliases

```ori
alias IntMap[V]   = map[int, V]
alias Callback[T] = func(T) -> bool
```

---

## Limitations in v1

The following generic features are **supported** in the current compiler.

### Associated types in traits

A trait may declare an associated `type` member that is resolved at
monomorphization time:

```ori
trait Container
    type Item
    get(self) -> Item
end
```

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

### Higher-kinded types (HKT)

Type constructors may appear as type parameters in constrained forms:

```ori
trait Functor<F[_]>
    fmap<A, B>(fa: F[A], f: func(A) -> B) -> F[B]
end
```

### Not supported in v1

- **Variadic type parameters**: `tuple[T...]` — not supported; use
  `tuple[A, B, ...]` with fixed arity.

These may be extended in future versions via explicit design decisions.

### Sanity tests

The syntax above is verified by `ori check` in `ori_spec.rs`:

- `generic_accepts_associated_type_in_trait` — `type Item` in a trait.
- `generic_accepts_const_generic_param` — `struct Matrix<const N: int>`.
- `generic_accepts_hkt` — `trait Functor<F[_]>`.
- `generic_accepts_where_constraint` — `for T: Comparable`.
- `generic_accepts_negative_constraint` — `for T: not Disposable`.
- `generic_accepts_generic_struct` — `struct Pair[A, B]`.
- `generic_accepts_type_inference` — type argument inferred from call site.
