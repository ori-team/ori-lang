# Ori Language Specification — Chapter 01: Overview

> Status: normative  
> Audience: language designers, compiler implementers, contributors  
> Surface: **S3** (Auk9-inspired), cutover `0.3.0` · living package **0.3.x** (FREEZE-1)  
> Naming: **snake_case** functions · **PascalCase** types · visibility **`public`**  
> Identity / purpose: [`00-manifesto.md`](00-manifesto.md)  
> Spec index / product facts: [`README.md`](README.md)

---

## What Ori Is

Ori is a statically typed, reading-first programming language compiled to native
code (AOT), with optional in-process JIT for `ori run`.

*ori* (אוֹרִי) — Hebrew for "my light."

Ori exists to make programming accessible to people who find mainstream languages
hostile — in particular people with ADHD, autism, and dyslexia — **and** as a
serious laboratory for compiler study and AI-assisted programming. It is **not**
a market-competition product. See the [manifesto](00-manifesto.md).

It achieves readability not by being simpler, but by being more honest: every
piece of information the reader needs is visible at the point where it is needed.

---

## What Ori Optimizes

Ori optimizes for **reading**, not writing.

A program is read many more times than it is written. Ori makes each read cheaper:

| Question | Visible through (S3) |
|---|---|
| Where does this file belong? | `module path` first in every file |
| What type does this value have? | Explicit annotations on bindings and public contracts |
| Can this value be absent? | `optional[T]` |
| Can this operation fail? | `result[T, E]` |
| When is a resource released? | `using` |
| Where does trait behavior come from? | `apply Type` + `use Trait` |
| What went wrong? | Structured diagnostic codes |

---

## Core Design Goals

1. **Explicit over implicit.** If something happens, you can see it in the source.
2. **No surprises.** The reader should predict runtime behavior from syntax alone.
3. **No null.** Absence is modeled as `optional[T]`.
4. **No exceptions as control flow.** Failure is modeled as `result[T, E]`.
5. **Composition over inheritance.** Types are composed with structs, enums, and traits.
6. **Readable diagnostics.** Every error message names what happened, where, and what to do.
7. **One canonical form per concept.** Dual legacy syntax is rejected at the `0.3.0` cut (S3).
8. **Accessible documentation.** Examples are short, syntactically valid, and up to date.

---

## What Ori Is Not

- Ori is not a scripting language. Programs have explicit structure.
- Ori is not a pure functional language. It supports functional patterns.
- Ori is not an object-oriented language. There are no classes or inheritance.
- Ori is not a systems language in the sense of manual memory management.
  Memory is managed automatically through value semantics and automatic reference counting.
- Ori is not a market product competing with industrial languages (see manifesto).

---

## Mental Model (S3)

An Ori program is a set of **modules**.

Each module is a source file. The module path is declared first with `module`.

```ori
module app.inventory

import ori.io = io

public item_count() -> int
    return 42
end
```

### Imports (three forms)

| Intent | Form | Effect |
|--------|------|--------|
| Selective | `import ori.fs (read_text, write_text)` | Names enter the local scope; rename with `item as alias` or `item = alias` |
| Module alias | `import ori.io as io` (or `= io`) | Use `io.print(...)` — **path left, alias right** |
| Whole module | `import ori.io` | Only fully-qualified `ori.io.print(...)` (no implicit alias) |

```ori
module app.api

public import app.inventory = inventory
```

```ori
module app.main

import app.api = api

main()
    const count: int = api.inventory.item_count()
end
```

Block form (multi-import with commas **only** inside the block):

```ori
imports
    ori.fs (read_text, write_text), ori.io = io
    app.users = users
end
```

Removed at `0.3.0` (hard errors): `namespace`, `import path only (…)`,
Auk9 order `import alias = path`. (Note: `import path as alias` is supported in 0.4+).

### Visibility

- Top-level declarations are private by default.
- `public` makes a declaration visible to other modules.
- `public import` re-exports; plain `import` does not.
- Accessing a private imported declaration emits `name.private`.

### The common path through Ori code

1. Define data shapes with `struct` and `enum`.
2. Define behavior contracts with `trait`.
3. Attach behavior with `apply Type` + `use Trait` (inline or `slot = freeFn`).
4. Return `optional[T]` when a value may be absent.
5. Return `result[T, E]` when an operation may fail; propagate with `try expr`.
6. Use `using` for deterministic cleanup.
7. Use `check` for programmer assertions.

---

## Complete Introductory Example

```ori
module app.main

import ori.io = io
import ori.core = core

alias UserResult = result[User, string]

struct User
    name: string
    age: int if it >= 0
end

apply User use core.Displayable
    display(self) -> string
        return f"{self.name} ({self.age})"
    end
end

load_user(id: int) -> UserResult
    if id < 0
        return err("invalid id")
    end
    return ok(User { name: "Ada", age: 36 })
end

main() -> result[void, string]
    const user: User = try load_user(1)
    io.print(user.display())
    return ok()
end
```

### Surface highlights in this example

| Concept | S3 form |
|---------|---------|
| File header | `module app.main` |
| Import alias | `import ori.io = io` |
| No `func` keyword | `load_user(...) -> …` / `main()` |
| Types | `result[User, string]`, brackets not angles |
| Struct literal | `User { name: …, age: … }` |
| Traits | compact header `apply User use core.Displayable` (one trait, nothing else) |
| Field contract | `age: int if it >= 0` — checked at runtime |
| Propagation | `try load_user(1)` only (`?` removed) |

---

## Surface S3 summary (breaking vs 0.2.x)

| Area | Canonical S3 | Removed |
|------|--------------|---------|
| Header | `module path` | `namespace` |
| Functions | `name(params) -> T` / `=> expr` | declaration `func` |
| Types | `list[T]`, `map[K,V]`, `optional[T]`, `result[T,E]` | `<>`, `of` / `to` forms |
| Generics | `Name[T]`, bounds `for T: Trait` | `where T is`, `func foo<T>` as canonical |
| Control | `elif`, `try expr` | `else if`, postfix `?` |
| Match | `case Variant` / `case Variant(...)` | leading `.` on case variants |
| Literals | `Type { f: v }`, `{ f: v }`, map `{ "k": v }` | `Type(...)`, `.{…}`, guided `(…)` |
| Imports | `path (A)`, `path = alias`, bare `path` | `as`, `only` |
| Traits | `apply Type` + `use Trait` | `implement Trait for Type`, `apply Trait to Type` |
| Closures | `(u) => expr` / `(u) … end` | `do(...)` |
| Rhythm | poetic call, labeled `end if` | nested poetic call |

Migration aid: `ori migrate-syntax`. Full list: `CHANGELOG.md` `[0.3.0]`.

**After 0.3.0 (delivered):**

| Slice | Content |
|-------|---------|
| **`0.3.1`** | Local Nim-style inference on obvious RHS (literals, typed struct/list forms) |
| **Option B** | Also omit on **field / index / call / pipe** with a concrete return type; reject `void` / `try` / empty `[]`/`{}` / bare `none` |
| **Pipe `\|\>`** | **Kept** as first-class Ori syntax (typed as `f(value)`) |
| **`0.3.8`** | High-performance wave: `@align(N)`, `@noalloc`, `simd[T, N]`, `mem.Region` arenas, and declarative `[native.dependencies]` |
| **`0.4.0`** | Surface ergonomics: `apply Type: Trait`, `import path as alias`, `array[T, N]`, inherent methods in `struct`, bare trait parameters |

**Still deferred / out of product:** multi-OS distribution and marketplace
publish (shelved until language, docs, and performance are solid); self-hosting
(M4) last.

---

## Relationship to Zenith / Auk9

Ori is a new language. Lessons from Zenith informed early design; source is not
compatible with Zenith.

| Historical / lab | Ori S3 |
|---|---|
| Zenith `text` | `string` |
| Zenith / early Ori `apply Trait to Type` | `apply Type` + `use Trait` |
| Early Ori `implement Trait for Type` | same as above |
| Early Ori `namespace` | `module` |
| Early Ori `func f(...)` | `f(...)` |
| Early Ori `list<T>` / `list of T` | `list[T]` |
| Auk9 (lab) surface | absorbed as S3 on Ori; Auk9 is **not** a product |
| Auk9 `import io = ori.io` | Ori uses `import ori.io = io` |
| Auk9 `do(u) =>` | Ori uses `(u) =>` |
| Ranges exclusive | Ori ranges are inclusive (`0..9` = 0–9) |
| `std.*` | `ori.*` |

---

## Spec Structure

| Chapter | Title |
|---|---|
| 00 | Manifesto (identity and purpose) |
| 01 | Overview (this chapter) |
| 02 | Lexical Structure |
| 03 | Formal EBNF Grammar |
| 04 | Type System |
| 05 | Expressions |
| 06 | Statements and Control Flow |
| 07 | Functions and Closures |
| 08 | Traits and Apply |
| 09 | Errors and Propagation |
| 10 | Memory and Cleanup |
| 11 | Generics and Constraints |
| 12 | Standard Library Contracts |
| 13 | Diagnostic Error Catalog |
| 14 | Backend Support Matrix |
| 15 | Stdlib Maintenance |
| 16 | Runtime FFI Safety |
| 17 | Project Layout and `.oridoc` |
| 18 | Stability and Compatibility |
| 19 | Native ABI (`ori-native-abi-1`) |

Chapter 03 is the standalone EBNF grammar in [`03-grammar.ebnf`](03-grammar.ebnf).
It is a compact syntax index and must be read together with the normative
lexical, type, expression, statement, function, and trait chapters. The
grammar does not replace the semantic rules in those chapters.
