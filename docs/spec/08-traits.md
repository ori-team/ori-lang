# Ori Language Specification — Chapter 08: Traits and Apply

> Status: normative
> Audience: compiler implementers, language designers
> Surface: **S3** (`0.3.0`)

---

## Overview

Traits describe behavior. They declare what a type must be able to do.
`apply Type` blocks attach trait behavior (and free methods) to a type via
`use Trait` sections.

Traits are Ori's mechanism for polymorphism. There is no class inheritance.

---

## Trait Declaration

```ori
trait Drawable
    draw(canvas: Canvas)
end

trait Serializable
    serialize() -> bytes
    deserialize(raw: bytes) -> result[Self, string]
end
```

A trait declares one or more **required** methods. A type applying the trait
must provide a concrete implementation for every required method (inline body
or bind).

---

## Default Methods

Traits may provide default implementations. **There is no `default` keyword**:
a method with a body is a default; a signature alone is required.

```ori
trait Displayable
    display(self) -> string

    print(self)
        io.print(self.display())
    end
end
```

- Methods with a body are **default methods**.
- Methods without a body are **required methods**.
- An applying type may override a default method.

---

## `Self` in Traits

`Self` inside a trait declaration refers to the concrete type that applies
the trait:

```ori
trait Cloneable
    clone() -> Self
end

trait Equatable
    equals(other: Self) -> bool
end
```

---

## `apply Type` + `use Trait` (S3)

When the whole block is one trait and nothing else, the **compact header** is
the required form:

```ori
apply Circle use Drawable
    draw(self, canvas: Canvas)
        canvas.draw_circle(self.center, self.radius)
    end
end
```

The nested form is required when the compact header cannot express the
content — two or more traits, or free members alongside a trait:

```ori
apply Circle
    area(self) -> float
        return 3.14159 * self.radius * self.radius
    end

    use Drawable
        draw(self, canvas: Canvas)
            canvas.draw_circle(self.center, self.radius)
        end
    end
end
```

**Which form applies is decided by the content, never by the writer.** A
nested block holding a single `use` and nothing else is rejected with
`apply.redundant_use_block` — it says exactly what the compact header says,
one indentation level deeper. `ori migrate-syntax` rewrites that shape
automatically.

The compact header is recognised by layout: `use` on the **same line** as
`apply` opens it; `use` on the next line is the nested form.

### Order (fixed)

1. Free methods and binds (`slot = freeFunction`) — optional; inherent-style on the type
2. Zero or more `use Trait` sections
3. Inside each `use`: associated types (`alias Name = Type`), then required
   slots and optional default overrides (inline or bind)

### Associated types

A `use` section may name types for its own signatures:

```ori
apply Bag use Container
    alias Item = string

    first_item(self) -> Item
        return self.label
    end
end
```

- Written with `alias`, the same word as a top-level alias, because it means
  the same thing: a transparent name for a type. Being inside a `use` section
  is what makes it associated.
- Scoped to that section: the name is undefined in a sibling `use`, in free
  members, and outside the `apply` block.
- The old `type Name = …` spelling was removed
  (`parse.associated_type_keyword_removed`); `ori migrate-syntax` rewrites it.

### Bind

Compile-time method provision via a free function (not a runtime assignment):

```ori
compare_points(a: Point, b: Point) -> int
    return a.x - b.x
end

apply Point use Comparable
    compare = compare_points
end
```

### Free methods without a trait

`apply Type` may contain only free methods/binds (no `use`). Those methods are
available as inherent methods on the type.

### Associated functions (no `self`)

**An explicit `self` parameter is what makes a method an instance method.**
A method declared without `self` — in an `apply` block or in a trait — is an
*associated function*: it has no receiver and is called through the type name:

```ori
apply User use core.Default
    default() -> User
        return User { name: "?", age: 0 }
    end
end

apply User
    make_empty() -> User          -- inherent associated function
        return User { name: "", age: 0 }
    end
end

const a: User = User.default()
const b: User = User.make_empty()
```

Rules:

- `self` inside an associated function is `bind.self_outside_method`.
- Calling one on a value (`u.default()`) is `type.assoc_fn_instance_call` —
  there is no receiver to pass.
- Associated trait functions are excluded from `any[Trait]` dispatch: dynamic
  dispatch needs a receiver's vtable.
- Bind slots (`compare = compare_points`) are unchanged: the bound free
  function receives the value as its first argument.

There is no implicit `self`: a body that uses `self` without declaring it does
not compile. (Earlier drafts tolerated `greet()` reading `self.name`; that
leniency was removed when associated functions landed.)

### Rules

- `apply Type` — the type receiving methods/traits.
- `use Trait` — attaches `Trait` to that type inside the apply block.
- All required methods from each used trait must be provided.
- Default methods may be omitted or overridden.
- Multiple traits may be used for the same type (one or several apply blocks).
- `self` may omit an explicit type annotation when the context is the applied type.
- Removed forms (hard error):
  - `implement Trait for Type` → `parse.implement_removed`
  - `apply Trait to Type` / `apply Trait for Type` → `parse.apply_trait_to_removed`

---

## `mut` in Traits and Apply

```ori
trait Counter
    mut increment(self, by: int)
    mut reset(self)
    total(self) -> int
end

struct Tally
    count: int
end

apply Tally use Counter
    mut increment(self, by: int)
        self.count = self.count + by
    end

    mut reset(self)
        self.count = 0
    end

    total(self) -> int
        return self.count
    end
end
```

`mut` on a trait method requires the applied method to also be `mut`. A method
without `mut` cannot assign to a field of `self`.

Note that collections have **no methods**: `list`, `map`, and `set` are
manipulated through their modules (`lists.push(items, v)`), not
`items.push(v)`. See chapter 12.

---

## Generic Traits

A trait may be generic over a type parameter, and an `apply` binds that
parameter positionally:

```ori
trait Container[Item]
    first(self) -> Item
end

apply IntBox use Container[int]
    first(self) -> int
        return self.v
    end
end

apply TextBox use Container[string]
    first(self) -> string
        return self.s
    end
end
```

The same trait can therefore be implemented at several types in one program.

Rules:

- The arguments are **required**: `use Container` on a generic trait reports
  `impl.trait_args_missing`, because the parameter would stay unbound and no
  implementation could match.
- The count must match the declaration — otherwise
  `impl.trait_arg_count_mismatch`.
- The implementation's signature is checked against the **bound** trait
  signature: `use Container[string]` with `first(self) -> int` is
  `impl.wrong_signature`.

Bounds on generic *functions* (`for T: Trait`) are a separate mechanism — see
chapter 11.

---

## Method resolution

- Inherent methods (struct body or free members of `apply Type`) take the path
  `namespace.Type.method`.
- Trait methods resolve via the impl table built from `use` sections; ambiguous
  names from multiple traits require qualification `Trait.method(receiver)`.
