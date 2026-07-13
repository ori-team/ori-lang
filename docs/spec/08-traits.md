# Ori Language Specification — Chapter 08: Traits and Apply

> Status: normative
> Audience: compiler implementers, language designers

---

## Overview

Traits describe behavior. They declare what a type must be able to do.
**`apply Trait to Type`** blocks attach trait behavior to a type.

Traits are Ori's mechanism for polymorphism. There is no class inheritance.

---

## Trait Declaration

```lua
trait Drawable
    func draw(canvas: Canvas)
end

trait Serializable
    func serialize() -> bytes
    func deserialize(raw: bytes) -> result<Self, string>
end
```

A trait declares one or more **required** methods. A type implementing the
trait must provide a concrete implementation for every required method.

---

## Default Methods

Traits may provide default implementations:

```lua
trait Displayable
    func display(self) -> string

    func print(self)
        io.print(self.display())
    end
end
```

- Methods with a body are **default methods**.
- Methods without a body are **required methods**.
- An implementing type may override a default method.

---

## `Self` in Traits

`Self` inside a trait declaration refers to the concrete type that implements
the trait:

```lua
trait Cloneable
    func clone() -> Self
end

trait Equatable
    func equals(other: Self) -> bool
end
```

---

## `apply` Blocks

```lua
apply Drawable to Circle
    func draw(canvas: Canvas)
        canvas.draw_circle(self.center, self.radius)
    end
end
```

Rules:
- `apply Trait to Type` — attaches `Trait` to `Type`.
- All required methods from the trait must be implemented.
- Default methods may be omitted (the trait's default is used) or overridden.
- `apply` blocks are not inside `struct` or `trait` declarations; they stand alone.
- Multiple traits may be applied to the same type.
- A trait may be applied to a type in any namespace that can see both.
- **Deprecated form (do not use in new code):** `implement Trait for Type`
  remains accepted and emits `parse.deprecated_implement`. Prefer
  `apply Trait to Type`.

---

## `mut func` in Traits and Apply

```lua
trait Stackable<T>
    mut func push(item: T)
    mut func pop() -> optional<T>
    func peek() -> optional<T>
end

apply Stackable<int> to IntStack
    mut func push(item: int)
        self.items.push(item)
    end

    mut func pop() -> optional<int>
        return self.items.pop()
    end

    func peek() -> optional<int>
        return self.items.last()
    end
end
```

`mut func` in a trait declaration requires the implementing function to also
be `mut func`.

---

## Generic Traits

Traits may be generic over a type parameter:

```lua
trait Container<Item>
    mut func add(item: Item)
    func get(index: int) -> optional<Item>
    func length() -> int
end
```

Applying a generic trait to a concrete type:

```lua
apply Container<string> to StringBag
    mut func add(item: string)
        self.items.push(item)
    end

    func get(index: int) -> optional<string>
        if index >= len(self.items)
            return none
        end
        return some(self.items[index])
    end

    func length() -> int
        return len(self.items)
    end
end
```

---

## Operator Traits

Current implementation status:

- Primitive numeric operators are implemented directly by the checker/codegen.
- String `+` is implemented as concatenation.
- User-defined `+`, `-`, `==`, `!=`, `<`, `<=`, `>`, and `>=` lower to
  trait methods when the concrete type implements the matching `ori.core`
  trait.
- The fixed trait set below is the supported contract.

Ori allows operator overloading only for the following fixed set:

| Operator | Trait | Method |
|---|---|---|
| `+` | `Addable` | `func add(other: Self) -> Self` |
| `-` | `Subtractable` | `func subtract(other: Self) -> Self` |
| `*` | `Multiplicable` | `func multiply(other: Self) -> Self` |
| `/` | `Divisible` | `func divide(other: Self) -> Self` |
| `<`, `<=`, `>`, `>=` | `Comparable` | `func compare(other: Self) -> int` |
| `==`, `!=` | `Equatable` | `func equals(other: Self) -> bool` |

No other operators will be overloadable. This is a deliberate design limit.

`Comparable.compare` follows the same sign convention used by sort callbacks:
return a negative integer when `self < other`, `0` when equal, and a positive
integer when `self > other`.

Behavior:

- `Comparable` provides `<`, `<=`, `>`, `>=` by deriving from `compare()`.
- `Equatable` provides `==` and `!=` by deriving from `equals()`.

---

## Standard Library Traits

Core traits defined in `ori.core`:

| Trait | Purpose |
|---|---|
| `Displayable` | `func display(self) -> string` — converts a value to string text |
| `Equatable` | Custom equality via `func equals(other: Self) -> bool` |
| `Comparable` | Ordering via `func compare(other: Self) -> int` |
| `Hashable` | Hash for map/set keys via `func hash() -> u64` |
| `Disposable` | Cleanup via `mut func dispose()` — used by `using` |
| `Iterable` | Iteration via `mut func next() -> optional<T>` |
| `Default` | Zero-argument construction via `func default() -> Self` |
| `From<Other>` | Explicit conversion from `Other` to `Self` |
| `Error` | Error display via `func message() -> string` |
| `Cloneable` | Explicit copy via `func clone() -> Self` |

---

## `Iterable` and `for` Loops

Any type implementing `core.Iterable` can be used in a `for` loop if its
implementation provides `mut func next() -> optional<T>`.

The item type is inferred from the `optional<T>` returned by `next`.

```lua
import ori.core as core

apply core.Iterable to CountUp
    mut func next() -> optional<int>
        if self.current > self.limit
            return none
        end
        const value: int = self.current
        self.current = self.current + 1
        return some(value)
    end
end

for n in CountUp(current: 1, limit: 5)
    io.print(string(n))  -- 1 2 3 4 5
end
```

Current limitation: generic `apply Iterable<int> to Type` syntax is not part of
the parser yet. Use `apply core.Iterable to Type` and let `next` define
the item type.

---

## `From<T>` — Explicit Conversion

```lua
apply From<int> to string
    func from(value: int) -> string
        return string(value)
    end
end

const s: string = string.from(42)
```

---

## `Error` Trait — Typed Errors

```lua
struct NetworkError
    code: int
    message: string
end

apply Error to NetworkError
    func message() -> string
        return f"Network error {self.code}: {self.message}"
    end
end

func fetch(url: string) -> result<bytes, NetworkError>
```

`Error` is required for a type to be used as the error branch of `result<T, E>`
in standard patterns. The compiler does not enforce this structurally (any type
may be an error), but stdlib functions expect `Error`-implementing types.

---

## Trait Resolution

When a method is called on a value, the compiler resolves the implementation:

1. Check the type's **inherent methods** (defined in `struct` block).
2. Check all `apply Trait to Type` blocks visible in scope.
3. If the method is unambiguous, call it.
4. If the method name matches two traits simultaneously: **compile error** (ambiguous).

**Disambiguation:**

```lua
-- If both Printable and Loggable define 'output()', use explicit trait call:
Printable.output(shape)
Loggable.output(shape)
```

---

## Overlapping Apply Blocks

Two `apply` blocks for the same `Trait`/`Type` pair in the same scope
are a **compile error**.

---

## `any<Trait>` — Dynamic Dispatch

`any<Trait>` is a dynamic trait object: a value whose concrete type is not
known at compile time, but which is guaranteed to implement `Trait`.

```lua
const shape: any<Drawable> = Circle(radius: 10.0)
shape.draw(canvas)
```

Only methods declared in `Trait` may be called through `any<Trait>`.
The concrete type is erased; the compiler generates a vtable.

`==` on `any<Trait>` uses runtime vtable dispatch: the concrete type's
`Equatable` implementation is invoked when both operands implement the trait
constraint. Prefer generics for performance-sensitive paths.

```lua
const a: any<core.Equatable> = 1
const b: any<core.Equatable> = 1
const same: bool = a == b   -- ok: structural equality via vtable
```

---

## Current implementation status

> Status: current as of 2026-06-27.

| Feature | Status | Sanity test |
| --- | --- | --- |
| Trait declaration (required + default methods) | implemented | `ori_spec.rs` — `trait_accepts_required_and_default_methods` |
| `Self` in trait signatures | implemented | `ori_spec.rs` — `generic_accepts_type_inference` |
| `apply Trait to Type` blocks | implemented | `method_resolution.rs` — `build_lowers_implement_method_call` |
| Missing required method rejected | implemented | `ori_spec.rs` — `trait_rejects_implement_missing_required_method` |
| `mut func` in traits/`apply` (`Disposable`) | implemented | `multifile_imports.rs` — `check_accepts_core_traits_and_using_core_disposable` |
| Generic traits (`Container<T>`) | implemented | `ori_spec.rs` — `generic_accepts_generic_struct` |
| HKT, associated types, const generics | implemented | `ori_spec.rs` — `generic_accepts_hkt`, `generic_accepts_associated_type_in_trait`, `generic_accepts_const_generic_param` |
| Operator traits (`Comparable`, `Equatable`, `Hashable`) | implemented | `collections.rs` — `check_accepts_hash_table_user_defined_hashable_equatable_key`, `check_rejects_heap_without_comparable_element` |
| `any<Trait>` dynamic dispatch + vtable | implemented | `ori_spec.rs` — `trait_accepts_any_dynamic_dispatch` |
| `==` / `!=` on `any<Trait>` via vtable | implemented | `ori_spec.rs` — `trait_object_equality_works` |
| `core.Iterable` + `for` loops | implemented | `collections.rs` — `compile_runs_custom_iterable_native`, `check_reports_non_iterable_for_loop` |
| Disambiguation via `Trait.method(value)` | implemented | `ori_spec.rs` — `trait_rejects_ambiguous_method_call` |
| Overlapping `apply` blocks | rejected (compile error) | `method_resolution.rs` — `check_reports_duplicate_implement_pair` |

The `any<Trait>` equality example above (lines 320–324) compiles and runs as
the `trait_object_equality_works` test: two `Circle` values with the same
radius compare equal through `any<Drawable>` vtable dispatch.
