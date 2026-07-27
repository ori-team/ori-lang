# Errors, optional, void — mental model

> Pedagogical guide (surface **S3 / 0.3.2**).  
> **Portuguese:** [errors-null-void.pt-BR.md](errors-null-void.pt-BR.md)  
> Normative: [09-errors](../spec/09-errors.md), [04-types](../spec/04-types.md)

## Four concepts

| Concept | Role | When |
|---------|------|------|
| **`void`** | No useful return | Side-effect functions |
| **`optional[T]`** | Value may be absent | Lookup, EOF — absence is not failure |
| **`result[T, E]`** | Success or failure with reason | I/O, validation |
| **`check`** | Runtime precondition | Invariants |

Ori has **no null**. Use `none` or `err(...)`.

## `void`

```ori
module app.main

import ori.io = io

greet() -> void
    io.println("hello")
end

main()
    greet()
end
```

## `optional[T]`

```ori
module app.main

find_user(id: int) -> optional[string]
    if id == 0
        return none
    end
    return some("alice")
end

main()
    match find_user(1)
        case some(name):
            -- use name
        case none:
    end
end
```

- Unpack with `if some(x) = expr` or `match`.
- `try` on optional propagates `none`.
- Postfix `?` was **removed** in S3.

## `result[T, E]`

```ori
module app.main

import ori.fs = fs

read_config(path: string) -> result[string, string]
    return fs.read_text(path)
end
```

- Build with **`ok(value)`** / **`err(reason)`** (not `success` / `error`).
- Handle with `match` or **`try expr`**.

## `check`

```ori
divide(a: int, b: int) -> int
    check b != 0, "division by zero"
    return a / b
end
```

Fails the process on broken contracts; it is not a `result`.

## Quick map

| Situation | Use |
|-----------|-----|
| Print only | `-> void` |
| “Not found”, not an error | `optional[T]` |
| Failure with message | `result[T, string]` |
| Must always be true | `check` |

```bash
ori explain name.undefined
ori doctor
```

Catalog: [13-error-catalog.md](../spec/13-error-catalog.md).
