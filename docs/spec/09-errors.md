# Ori Language Specification — Chapter 09: Errors and Propagation

> Status: normative
> Audience: compiler implementers

---

## Overview

Ori has two distinct constructs for absence and failure:

| Construct | Use | Propagation |
|---|---|---|
| `optional<T>` | A value that may be absent | `try`, `?`, or `if some` |
| `result<T, E>` | An operation that may fail | `try` or `?` |

There are no exceptions. There is no `null`. There is no `throw`/`catch`.

All failure paths are visible in function signatures and in the code that
handles them.

---

## `optional<T>`

`optional<T>` represents a value that may or may not be present.

```lua
func find_user(id: int) -> optional<User>
    if id <= 0
        return none
    end
    return some(database.lookup(id))
end
```

Constructors:
- `some(value)` — wraps a value.
- `none` — the absent value.

### Unwrapping `optional<T>`

**Pattern match** (explicit, always safe):

```lua
match find_user(42)
case some(user):
    greet(user)
case none:
    io.print("not found")
end
```

**`if some` binding** (concise single-branch handling):

```lua
if some(user) = find_user(42)
    greet(user)
end
```

**`.or(fallback)`** — unwrap or use a default:

```lua
const name: string = find_name(id).or("Anonymous")
```

Current status: `.or(fallback)` is accepted for `optional<T>` and
`result<T, E>` in the checker, native backend, and C backend. The fallback is
evaluated only when the receiver is `none` or `error(_)`.

**`.or_return()`** — unwrap or propagate from the enclosing function:

```lua
const user: User = find_user(id).or_return()
-- If find_user returns none, the enclosing function returns none immediately.
-- The enclosing function must return optional<_>.
```

Current status: `.or_return()` is accepted as shorthand for propagation. The
older `.or_return(value)` form is not implemented. Use `try`, `?`,
`if some(...) = ...`, or `match` when explicit control flow is clearer.

**`try` propagation** — unwrap or propagate absence:

```lua
func get_user_name(id: int) -> optional<string>
    const user: User = try find_user(id)
    -- If none: return none from get_user_name
    -- If some(u): bind u to user
    return some(user.name)
end
```

`find_user(id)?` is the compact form with the same semantics.

---

## `result<T, E>`

`result<T, E>` represents an operation that either succeeds with value `T` or
fails with error `E`.

```lua
func read_config(path: string) -> result<Config, string>
    if path == ""
        return error("empty path")
    end
    const raw: string = try ori.fs.read_text(path)
    return success(try parse_config(raw))
end
```

Constructors:
- `success(value)` — the success variant.
- `error(value)` — the failure variant.

### `try` Propagation on `result<T, E>`

```lua
func start(path: string) -> result<void, string>
    const config: Config = try read_config(path)
    -- If error(e): return error(e) from start
    -- If success(v): bind v to config
    apply_config(config)
    return success()
end
```

Rules for `try` on `result<T, E>`:
1. The enclosing function must return `result<_, F>` where `F` is compatible with `E`.
2. If `E == F`: the error is propagated as-is.
3. If `E != F`: a compile error. Use explicit conversion. For `result<T, string>`,
   `.or_wrap(context)` can add string context before propagation.

`read_config(path)?` is the compact form with the same semantics.

### `.or_wrap(context)`

Adds a context string to an existing error without losing the original:

```lua
const config: Config = try read_config(path).or_wrap("loading configuration")
```

If `read_config` returns `error("empty path")`, the result becomes
`error("loading configuration: empty path")`.

Current status: `.or_wrap(...)` is accepted for `result<T, string>` in the
checker, HIR lowering, native backend, and C backend. It keeps `success(v)`
unchanged and evaluates the context expression only when the receiver is
`error(_)`. For non-string error types, use explicit conversion or handle the
error with `match`.

### Pattern Match on `result<T, E>`

```lua
match load_data(path)
case success(data):
    process(data)
case error(msg):
    io.print(f"failed: {msg}")
end
```

---

## `try` and `?` — Propagation

`try expr` is the readable propagation form. It works on both `optional<T>` and
`result<T, E>`.

`expr?` is the compact form with the same semantics. Prefer `try` in examples,
guides, public docs, and code where readability is more important than brevity.

**Behavior summary:**

| Type | On success | On failure |
|---|---|---|
| `optional<T>` | Unwraps to `T` | Returns `none` from enclosing function |
| `result<T, E>` | Unwraps to `T` | Returns `error(e)` from enclosing function |

**Compatibility rules:**

The enclosing function's return type must be compatible:

| Expression type | Required enclosing return type |
|---|---|
| `try optional<T>` / `optional<T>?` | `optional<_>` |
| `try result<T, E>` / `result<T, E>?` | `result<_, E>` or `result<_, F>` where `E` converts to `F` |

Using `try` or `?` in a function that returns `void` or an incompatible type is
a compile error.

Backend status:

- The native backend supports `try` and `?` propagation.
- The C backend supports `try` and `?` propagation for `optional<T>` and
  `result<T, E>` when the enclosing function returns a compatible
  `optional<_>` or `result<_, E>`.

---

## Error Types

Any type may be used as the error branch of `result<T, E>`.

Current implementation: `ori.core.Error` exists as a marker trait. The richer
`message()`/`cause()` trait-method contract below is planned and documents the
intended stable shape for future stdlib APIs:

```lua
trait Error
    func message() -> string

    func cause() -> optional<any<Error>>
        return none
    end
end
```

Using `Error` is not required by the compiler today. Future rich stdlib APIs may
require it. The current `ori.Error` value type is importable and stores
`code: string` plus `message: string`; current `ori.io`/`ori.fs` helpers still
use `string` errors or `optional<T>` where documented.

### Defining Error Types

Use `struct` + `apply Error to`:

```lua
struct ValidationError
    field: string
    reason: string
end

apply Error to ValidationError
    func message() -> string
        return f"validation failed on '{self.field}': {self.reason}"
    end
end
```

### Error Union with `enum`

When a function may fail with multiple distinct error types, use an enum:

```lua
enum AppError
    Network(error: NetworkError)
    Validation(error: ValidationError)
    Parse(error: ParseError)
end

apply Error to AppError
    func message() -> string
        match self
        case .Network(error):
            return error.message()
        case .Validation(error):
            return error.message()
        case .Parse(error):
            return error.message()
        end
    end
end

func run(input: string) -> result<Output, AppError>
```

This guarantees exhaustive handling at the call site.

---

## Panic

A panic is a non-recoverable error. It terminates the program (or the current
thread/job, depending on the runtime).

Sources of panic:
- `panic("message")` — explicit panic.
- `check condition` — assertion failure.
- `todo()` — unimplemented path reached.
- `unreachable()` — supposedly unreachable path reached.
- Integer division by zero.
- Index out of bounds.
- Field contract violation.
- Failed type narrowing (`is` check followed by wrong-type access).

Panics are not catchable at the language level. Use `result<T, E>` for
expected failures.

---

## No Exceptions

Ori has no exceptions, no `throw`, no `catch`, no exception-style `try` blocks.
The `try expr` syntax is not a catch block; it is only readable propagation for
`optional<T>` and `result<T, E>`.

The reason: exceptions are invisible in function signatures. A function that
throws can fail in ways not visible to the caller, requiring knowledge of the
implementation to handle correctly. In Ori, every function that can fail
says so in its return type.

---

## Common Patterns

### Chaining fallible operations

```lua
func process(path: string) -> result<Output, string>
    const raw: string = try ori.fs.read_text(path)
    const parsed: Input = try parse(raw)
    const output: Output = try transform(parsed)
    return success(output)
end
```

### Converting error types

```lua
func run(path: string) -> result<void, AppError>
    -- result<Config, string> is not result<void, AppError>.
    -- Explicit conversion is needed today.
    match read_config(path)
    case success(c):
        apply_config(c)
        return success()
    case error(msg):
        return error(AppError.Parse(ParseError(message: msg)))
    end
end
```

### Early return from nested optional

```lua
func find_display_name(id: int) -> optional<string>
    const user: User = try find_user(id)
    const profile: Profile = try find_profile(user.profile_id)
    return some(profile.display_name)
end
```
