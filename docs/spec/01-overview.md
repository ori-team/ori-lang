# Ori Language Specification — Chapter 01: Overview

> Status: normative  
> Audience: language designers, compiler implementers, contributors  
> Current project version: **0.3.8**  
> Language surface: **S3**  
> Naming: **snake_case** functions · **PascalCase** types · visibility **`public`**  
> Identity and purpose: [`00-manifesto.md`](00-manifesto.md)  
> Spec index: [`README.md`](README.md)

---

## What Ori is

Ori is a statically typed, reading-first programming language compiled to native code through Cranelift. `ori run` may execute through an in-process JIT when a compatible runtime cdylib is available.

*ori* (אוֹרִי) means “my light” in Hebrew.

Ori exists as:

- a serious compiler-engineering project;
- a language for learning and building native programs;
- a laboratory for AI-assisted programming;
- an attempt to reduce cognitive load through visible contracts and actionable diagnostics.

Ori is currently pre-1.0. Public contracts and experimental areas are separated by [`18-stability-and-compatibility.md`](18-stability-and-compatibility.md).

---

## What Ori optimizes

Ori optimizes for reading before writing.

A reader should find important information near the code that depends on it.

| Question | Visible through |
|---|---|
| Where does this file belong? | `module path` at the beginning of the file |
| What type does this value have? | explicit annotations and documented local inference |
| Can this value be absent? | `optional[T]` |
| Can this operation fail? | `result[T, E]` |
| How is failure propagated? | `try expression` |
| When is a resource released? | `using` and documented destructor behavior |
| Where does trait behavior come from? | `apply Type` and `use Trait` |
| What went wrong? | stable diagnostic codes with labels and actions |

Readability does not mean avoiding advanced features. It means making their contracts inspectable.

---

## Core design goals

1. **Explicit over hidden.** Important behavior should be visible in source or a local contract.
2. **One canonical form.** A concept should not require choosing among equivalent legacy spellings.
3. **Predictable failure.** Recoverable failure uses `result[T, E]`, not exceptions as ordinary control flow.
4. **Explicit absence.** Absence uses `optional[T]`, not a universal null value.
5. **Composition.** Structs, enums, traits, and functions compose behavior without class inheritance.
6. **Readable diagnostics.** Errors identify the problem, location, reason, and likely action when useful.
7. **Native execution.** The native Cranelift backend is the semantic reference.
8. **Accessible documentation.** Active examples are current, focused, and executable where presented as runnable.
9. **Versioned interoperability.** Runtime layouts and symbols are governed by a separate native ABI contract.

---

## What Ori is not

- Ori is not dynamically typed.
- Ori is not a pure functional language, although it supports functional patterns.
- Ori is not class-based or inheritance-oriented.
- Ori does not expose Rust-style manual borrowing or explicit lifetime syntax.
- Ori is not a bytecode/VM-first product.
- Ori is not self-hosted at the current stage.
- Ori's managed memory model does not remove the need for explicit resource cleanup.

---

## Program model

An Ori program is a graph of modules.

Each source file begins with a module declaration:

```ori
module app.inventory
```

A project normally has `ori.proj` at its root and an entry file such as `main.orl`. Project and documentation formats are defined in [`17-project-and-docs.md`](17-project-and-docs.md).

### Minimal program

```ori
module app.hello

import ori.io = io

main()
    io.print("Hello, Ori!")
end
```

---

## Declarations and visibility

Top-level declarations are private by default. `public` exposes a declaration to other modules.

```ori
module app.inventory

public item_count() -> int
    return 42
end
```

Public contracts require explicit types according to the relevant chapters. Local inference is limited to documented forms; it is not global Hindley–Milner inference.

---

## Imports

Ori supports three canonical import forms.

| Intent | Form | Effect |
|---|---|---|
| Select names | `import ori.fs (read_text, write_text)` | selected names enter local scope |
| Alias a module | `import ori.io = io` | module is accessed through `io` |
| Import whole path | `import ori.io` | access remains fully qualified |

Selective imports may rename an item:

```ori
import ori.fs (read_text = read)
```

Public imports re-export:

```ori
module app.api

public import app.inventory = inventory
```

Removed forms such as `namespace`, `import path as alias`, and `import path only (...)` are rejected. Migration assistance is available through `ori migrate-syntax` for supported mechanical rewrites.

---

## Types and values

Common type forms include:

```text
int
float
bool
string
bytes
list[T]
map[K, V]
optional[T]
result[T, E]
func(A, B) -> R
```

Structs and enums define user data.

```ori
struct User
    name: string
    age: int
end

enum LoadError
    NotFound
    Invalid(message: string)
end
```

Construction uses visible type and field/variant syntax:

```ori
const user: User = User { name: "Ada", age: 36 }
const failure: LoadError = LoadError.Invalid("bad record")
```

Context-typed shorthand is allowed only where the expected type is known and documented.

---

## Failure and absence

Use `optional[T]` when a value may be absent.

Use `result[T, E]` when an operation may fail and the caller must handle or propagate the failure.

```ori
module app.math

import ori.io = io

divide(a: int, b: int) -> result[int, string]
    if b == 0
        return err("division by zero")
    end

    return ok(a / b)
end

main() -> result[void, string]
    const answer: int = try divide(84, 2)
    io.print(f"answer: {answer}")
    return ok()
end
```

`try expression` is the canonical propagation form. Removed postfix propagation forms are rejected.

---

## Traits and behavior

Traits define behavior contracts. Implementations use `apply Type` and one or more `use Trait` sections according to the grammar in [`08-traits.md`](08-traits.md).

```ori
module app.user

import ori.core = core

struct User
    name: string
end

apply User use core.Displayable
    display(self) -> string
        return self.name
    end
end
```

An explicit `self` parameter defines an instance method. Receiver-less trait methods follow the associated-function rules documented in the trait specification.

---

## Resource cleanup

`using` provides deterministic cleanup for values implementing the required disposal contract.

Object destructors and ARC cleanup follow separate runtime rules. Deterministic resource cleanup, custom destruction, and managed-memory release must not be treated as interchangeable.

See:

- [`10-memory.md`](10-memory.md);
- [`16-runtime-ffi-safety.md`](16-runtime-ffi-safety.md);
- [`19-abi.md`](19-abi.md).

---

## Execution model

The current compiler pipeline is:

```text
source/project
  -> lexer
  -> parser and AST
  -> definitions and name resolution
  -> type checking
  -> typed HIR
  -> optimization
  -> Cranelift AOT or JIT
  -> versioned native runtime
```

The native backend defines reference semantics. The C/debug backend supports a documented subset and must reject unsupported features explicitly.

Backend support is defined in [`14-backend-support.md`](14-backend-support.md).

---

## Diagnostics

Diagnostics use stable codes such as:

```text
name.undefined
parse.unterminated_block
type.arg_count_mismatch
```

Every emitted public code belongs in [`13-error-catalog.md`](13-error-catalog.md). CLI and LSP may render diagnostics differently but must preserve their semantic identity and source ranges.

---

## Complete introductory example

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

This example demonstrates:

- module identity;
- module aliases;
- explicit data and public contracts;
- field contracts;
- trait implementation;
- result construction and propagation;
- native-runtime output.

---

## Surface history

The S3 surface was introduced in the 0.3 line and remains the current language surface in Ori 0.3.8.

Historical introduction versions may be documented in the changelog, but active status documents use the current project version.

The S3 design absorbed lessons from the retired Auk9 language-design laboratory. Auk9 is not a separate current product or compatibility target.

---

## Where to continue

- Modules and declarations: the next specification chapters.
- Types: [`04-types.md`](04-types.md).
- Expressions: [`05-expressions.md`](05-expressions.md).
- Statements and control flow: [`06-statements.md`](06-statements.md).
- Traits: [`08-traits.md`](08-traits.md).
- Errors: [`09-errors.md`](09-errors.md).
- Memory: [`10-memory.md`](10-memory.md).
- Generics: [`11-generics.md`](11-generics.md).
- Standard library: [`12-stdlib.md`](12-stdlib.md).
- Stability: [`18-stability-and-compatibility.md`](18-stability-and-compatibility.md).
- ABI: [`19-abi.md`](19-abi.md).