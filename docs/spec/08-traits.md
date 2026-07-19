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
3. Inside each `use`: required slots and optional default overrides (inline or bind)

### Bind

Compile-time method provision via a free function (not a runtime assignment):

```ori
comparePoints(a: Point, b: Point) -> int
    return a.x - b.x
end

apply Point use Comparable
    compare = comparePoints
end
```

### Free methods without a trait

`apply Type` may contain only free methods/binds (no `use`). Those methods are
available as inherent methods on the type.

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
trait Stackable[T]
    mut push(self, item: T)
    mut pop(self) -> optional[T]
    peek(self) -> optional[T]
end

apply IntStack
    use Stackable[int]
        mut push(self, item: int)
            self.items.push(item)
        end

        mut pop(self) -> optional[int]
            return self.items.pop()
        end

        peek(self) -> optional[int]
            return self.items.last()
        end
    end
end
```

`mut` on a trait method requires the applied method to also be `mut`.

---

## Generic Traits

Traits may be generic over a type parameter (`Trait[T]`). Bounds use
`for T: Trait` on generic methods (see chapter 11).

---

## Method resolution

- Inherent methods (struct body or free members of `apply Type`) take the path
  `namespace.Type.method`.
- Trait methods resolve via the impl table built from `use` sections; ambiguous
  names from multiple traits require qualification `Trait.method(receiver)`.
