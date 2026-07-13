# Ori Language Specification — Chapter 07: Functions and Closures

> Status: normative
> Audience: compiler implementers

---

## Function Declarations

```lua
func add(a: int, b: int) -> int
    return a + b
end

pub func greet(name: string) -> string
    return f"hello {name}"
end
```

Rules:
- `func` declares a named function.
- Parameter types and return type are explicit.
- `pub` makes the function visible outside its namespace.
- Private (default) functions are visible only within their namespace.
- Functions close with `end`.
- A function that returns `void` may omit the `-> void` annotation.

---

## Async Functions

```lua
import ori.task as task

async func load_count() -> int
    await task.sleep(1)
    return 41
end

async func main()
    const count: int = await load_count()
end
```

Rules:
- `async func f(...) -> T` has call type `future<T>`.
- Inside the function body, `return value` still returns a `T`.
- `await expr` may appear only inside an `async func`.
- `await` requires `expr` to have type `future<T>` and produces `T`.
- `async func main()` is allowed in the native backend. The generated entry
  point waits for its returned future through the native executor.
- `using` is allowed inside `async func`. Resources are stored in the async
  frame and `dispose()` runs on scope exit. Full coverage of every terminal path
  (cancellation, early exit) is tracked in the implementation master plan.

Implementation status:
- The native runtime has pollable futures, failed/cancelled internal states,
  continuation scheduling, a FIFO executor queue, and non-blocking timers for
  `task.sleep`.
- Current native lowering creates a `future<T>` as soon as an `async func` is
  called. Supported bodies are lowered to a generated native state machine and
  scheduled on the native executor.
- Source-level `await` in the supported subset uses `ori_future_poll`,
  `ori_future_value_*`, and `ori_future_on_ready`; it does not use
  `task.block_on`.
- Async shapes outside the current state-machine subset fail with
  `backend.native_unsupported` before Cranelift.
- Failed and cancelled future states observed by the state machine are
  propagated by generated async wrappers instead of becoming silent default
  values.
- Public cooperative cancellation via `task.CancelToken` (`create_token`,
  `cancel`, `is_cancelled`, `associate`).

---

## Parameters

### Required Parameters

```lua
func connect(host: string, port: int) -> result<void, string>
```

### Default Values

```lua
func connect(host: string, port: int = 80) -> result<void, string>
```

- Parameters with defaults must come after required parameters.
- The default expression is evaluated at each call site.

### Named Arguments

At the call site, arguments may be named:

```lua
connect(host: "localhost", port: 8080)
```

Rules:
- Once a named argument is used, all subsequent arguments must also be named.
- Named arguments may be given in any order.
- For `pub` functions, parameter names are part of the external API.

### Value Contracts (`if` on parameters)

```lua
func sqrt(value: float if it >= 0.0) -> float
func clamp(v: int, lo: int, hi: int if it >= lo) -> int
```

`it` is the contextual keyword that refers to the parameter value being checked.
The contract is evaluated at every call site. A violation is a runtime panic
(`contract.param_violation`).

### Variadic Parameters

The last parameter may accept zero or more values of its type:

```lua
pub func log(prefix: string, values: any<Displayable>...)
```

- `values` is typed as `list<any<Displayable>>` inside the function body.
- Only the last parameter may be variadic.
- At the call site, pass values directly:

```lua
log("info", count, name, active)
```

- To spread a list into a variadic: use `..`:

```lua
const parts: list<string> = ["a", "b", "c"]
concat(..parts)
```

---

## Return Types

### Explicit Return

```lua
func area(w: int, h: int) -> int
    return w * h
end
```

### Void Return

```lua
func print_all(items: list<string>)
    for item in items
        io.print(item)
    end
end
```

When a function returns `void`, `return` with no value exits early.

### `result<T, E>` and propagation

Most functions that can fail return `result<T, E>`:

```lua
func read_file(path: string) -> result<string, string>
    const file: ori.fs.File = try ori.fs.open_read(path)
    return ori.fs.read_all(file)
end
```

See Chapter 09 — Errors and Propagation for full semantics.

---

## Mutating Methods (`mut func`)

When a function modifies the state of `self`, it must be declared `mut func`:

```lua
struct Counter
    value: int

    mut func increment()
        self.value = self.value + 1
    end

    func get() -> int
        return self.value
    end
end
```

Rules:
- `mut func` may assign to `self` or its fields.
- A non-`mut` function may not modify `self`.
- Calling a `mut func` on a `const` binding is a compile error:

```lua
const c: Counter = Counter(value: 0)
c.increment()    -- Error: cannot call mut func on const binding

var c: Counter = Counter(value: 0)
c.increment()    -- OK
```

---

## Methods on Structs

Functions declared inside a `struct` block are methods. They receive an
implicit `self` parameter of the struct type.

```lua
struct Rectangle
    width: float
    height: float

    func area() -> float
        return self.width * self.height
    end

    func scale(factor: float) -> Rectangle
        return Rectangle(
            width: self.width * factor,
            height: self.height * factor,
        )
    end
end
```

Methods declared in a `struct` block are not required to be in an `apply`
block (they are "inherent methods").

---

## Closures (`do`)

Closures are anonymous functions. They use `do` instead of `func`.

### Inline Closure (expression body)

```lua
const double: func(int) -> int = do(x: int) => x * 2
const is_even: func(int) -> bool = do(n: int) => n % 2 == 0
```

Syntax: `do(params) => expression`

The return type is inferred from the expression type. An explicit return type
may be provided:

```lua
do(x: int) -> int => x * 2
```

### Block Closure (statement body)

```lua
const process: func(string) -> bool = do(input: string)
    const trimmed: string = input.trim()
    return len(trimmed) > 0
end
```

Syntax: `do(params) [ -> return_type ] block`

### Closures as Arguments

The most common use is passing closures to higher-order functions:

```lua
const doubled: list<int> = iter.map(numbers, do(x: int) => x * 2)
const valid: list<string> = iter.filter(names, do(n: string) => len(n) > 0)
```

When the closure type can be inferred from the function signature, the
return type annotation may be omitted:

```lua
iter.map(numbers, do(x: int) => x * 2)
-- The return type of `do(x: int) => x * 2` is inferred as int
-- from the expected type `func(int) -> int`
```

### Capture Rules

Closures capture values from their enclosing scope by **value** (copy):

```lua
const prefix: string = "Dr. "
const greet: func(string) -> string = do(name: string) => f"{prefix}{name}"
-- prefix is captured by copy at the time do(...) is evaluated
```

Capture rules:
- `const` bindings: captured by copy (always safe).
- `var` bindings: **compile error** — closures may not capture mutable bindings.
  Extract the current value first:

```lua
var counter: int = 0
-- Error: cannot capture var binding in closure
-- const snapshot: func() -> int = do() => counter

-- Correct: capture the current value
const current: int = counter
const snapshot: func() -> int = do() => current
```

### Closures vs Named Functions

For complex logic, prefer a named function:

```lua
func is_valid_name(name: string) -> bool
    if len(name) == 0
        return false
    end
    return len(name) <= 100
end

const valid_names: list<string> = iter.filter(names, is_valid_name)
```

Named functions can be passed directly where a `func(T) -> R` is expected.

---

## Higher-Order Functions

Functions may accept and return callable values:

```lua
func apply_twice(value: int, f: func(int) -> int) -> int
    return f(f(value))
end

const result: int = apply_twice(5, do(x: int) => x * 2)  -- 20
```

---

## Generic Functions

See Chapter 11 — Generics and Constraints for full specification.

```lua
func identity<T>(value: T) -> T
    return value
end

func first<T>(items: list<T>) -> optional<T>
    if len(items) == 0
        return none
    end
    return some(items[0])
end
```

---

## `self` Parameter

`self` refers to the receiver in method and `apply` block functions.
It is always the implicit first parameter and is never written in the
parameter list.

`self` is `const` by default. In a `mut func`, `self` is mutable.

---

## `check`, `todo`, `unreachable`, `panic`

These are special contextual forms, not regular functions:

```lua
check condition               -- assert condition; panic if false
check condition, "message"    -- with custom panic message

todo()                        -- marks unimplemented code; always panics
todo("message")

unreachable()                 -- asserts this point is never reached; panics if it is
unreachable("message")

panic("fatal error")          -- unconditional panic with message
```
