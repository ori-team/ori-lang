# Ori Language Specification — Chapter 06: Statements and Control Flow

> Status: normative
> Audience: compiler implementers
> Surface: **S3** (`0.3.0`)

---

## Overview

Statements produce effects. They appear inside function bodies (blocks).
A block is a sequence of statements closed by `end`.

---

## Binding Declarations

### `const`

Declares an immutable binding.

```ori
const name: string = "Ada"
const max: int = 100

-- 0.3.1 + option B: omit type when the RHS is obvious on the same line
const n = 1
const label = "ready"
const user = User { name: "Ada", age: 36 }
const xs = [1, 2, 3]
const title = user.name          -- field
const first = xs[0]              -- index
const upper = str.to_upper("hi") -- call with known return
const twice = 21 |> double       -- pipe (= double(21))
```

- Cannot be reassigned after declaration.
- Type annotation is **required** on top-level / `pub` bindings and whenever the
  RHS is not locally obvious; locals may omit it (`0.3.1` + option B).
- Failure to infer a local type → `type.local_inference_failed`.
- **Must annotate** (not inferable): `try expr`, empty `[]`/`{}`, bare `none`,
  `void` results, non-concrete types, and types only recoverable from *later*
  uses in the block.
- The initializer is evaluated once, at declaration time.

### `var`

Declares a mutable binding.

```ori
var count: int = 0
count = count + 1

var total = 0          -- 0.3.1 + B local inference
var name = user.name   -- option B field
```

- Can be reassigned with `=`.
- Type annotation rules match `const` (required unless local Nim-style + B
  inference applies).
- Compound assignment operators are available: `+=`, `-=`, `*=`, `/=`.

---

## Assignment

```ori
count = 42
user.name = "Bo"
items[0] = 99
```

The left-hand side must be a mutable binding or a mutable path through a `var`.
Assigning to a `const` binding is a compile error.

---

## Return

```ori
return value
return          -- implicit void
```

`return` exits the enclosing function. If the function declares a return type,
the returned expression must match.

A function body that reaches the end without `return` is a compile error if
the return type is not `void`.

---

## `if` / `else`

```ori
if count > 0
    process()
end

if name == "Ada"
    greet_ada()
else
    greet_other()
end

if score >= 90
    io.print("A")
elif score >= 80
    io.print("B")
else
    io.print("C")
end
```

Chained conditionals use `elif` (S3). The form `elif` is rejected with
`parse.else_if_removed`.

The condition must be `bool`. There is no truthy/falsy coercion.

Inline if-expression (expression context, not a statement) keeps the Ori form
`if cond then a else b` — see chapter 05.

---

## `if some` — Optional Binding

```ori
if some(user) = get_user(id)
    greet(user)
end

if some(user) = get_user(id)
    greet(user)
else
    io.print("not found")
end
```

- Evaluates `get_user(id)`.
- If `some(v)`: binds `v` to `user` and executes the block.
- If `none`: skips to `else` block (or exits if no `else`).
- `user` is scoped to the `if` block only.

---

## `if ok` / `if err` — Result Binding

The same form for `result[T, E]`: `ok` binds the success value, `err` binds
the error value.

```ori
if ok(value) = divide(10, 2)
    io.print(string(value))
else
    io.print("failed")
end

if err(message) = divide(1, 0)
    io.print(message)
end
```

- `if ok(v) = expr` takes the branch when the result is `ok`, binding `T`.
- `if err(e) = expr` takes the branch when the result is **not** ok,
  binding `E`.
- The scrutinee must be a `result[T, E]` (`type.ifok_not_result` /
  `type.iferr_not_result` otherwise).
- The binding is scoped to the `if` block only, as with `if some`.

Use `try` when the intent is to propagate the error upward; use `if ok` /
`if err` when this function handles it locally.

---

## `while`

```ori
while count < 10
    count += 1
end
```

The condition must be `bool`. Evaluated before each iteration.

---

## `while some` — Optional Loop

```ori
while some(line) = reader.next_line()
    process(line)
end
```

Continues as long as `next_line()` returns `some(v)`. Stops on `none`.
`line` is scoped to each iteration of the loop body.

---

## `loop` — Infinite Loop

```ori
loop
    const input: string = try console.read_line()
    if input == "quit"
        break
    end
    process(input)
end
```

`loop` runs forever until `break` is reached or the function returns.
Use `break` to exit. Use `continue` to skip to the next iteration.

---

## `for` — Iteration

```ori
for item in items
    process(item)
end
```

Iterates over built-in iterables or a value that implements `core.Iterable`.

Built-in iterables: `list[T]`, `map[K, V]`, `set[T]`, `string`, `bytes`,
and `range[int]`.

Custom iterable contract:

- import the core trait module, usually `import ori.core = core`;
- implement `core.Iterable` for the concrete type;
- provide `mut next() -> optional[T]`;
- the `for` binding has type `T`;
- the second binding is the zero-based `int` index.

**With index:**

```ori
for item, index in items
    io.print(f"{index}: {item}")
end
```

For `list[T]`, `set[T]`, `string`, `bytes`, ranges, and custom iterables:
second binding is the `int` index.
For `map[K, V]`: second binding is the value `V` (first is the key `K`).

**Range iteration:**

```ori
for i in 0..9
    io.print(string(i))
end

for i in 9..0          -- descending: 9, 8, ..., 0
    io.print(string(i))
end
```

---

## `repeat` — Fixed Count

```ori
repeat 3
    attempt()
end

repeat 3 times     -- `times` is optional for readability
    attempt()
end
```

The count expression is evaluated once. Must be integral (`int` or unsigned int).
Zero produces no iterations. Negative value is a runtime panic.
`times` is a contextual keyword: only special after `repeat expression`;
otherwise it is a valid identifier.

---

## `match` — Pattern Matching

```ori
match shape
case Circle(radius):
    draw_circle(radius)
case Rectangle(w, h):
    draw_rect(w, h)
case Point:
    draw_point()
end
```

Rules:
- `match` must be exhaustive: every possible value must be covered.
- `case else:` is the explicit fallback for non-exhaustive cases.
- Guarded cases (`case p if cond:`) do not count toward exhaustiveness.
- Cases are evaluated in source order; the first matching case executes.
- Bindings in a case are scoped to that case's body and guard.
- Unreachable cases produce a compile warning.

**Exhaustiveness requirement:**
```ori
-- Error: non-exhaustive match (missing case for 'Point')
match shape
case Circle(r):
    draw_circle(r)
end

-- Correct with case else:
match shape
case Circle(r):
    draw_circle(r)
case else:
    draw_default()
end
```

**Alternatives (`or` patterns):**

```ori
match direction
case North or South:
    move_vertical()
case East or West:
    move_horizontal()
end
```

- `case a or b:` matches when any alternative matches.
- Alternatives **may not bind values** (`match.or_pattern_binding`): each
  branch would otherwise have to bind the same names at the same types.
  Use one `case` per alternative, or a guard, when the payload is needed.
- For coverage, `case a or b:` counts exactly as `case a:` plus `case b:`.
- The separator is the word `or`, matching the boolean operators; `|` is not
  a token in Ori.

**Guard:**

```ori
match score
case n if n >= 90:
    io.print("A")
case n if n >= 80:
    io.print("B")
case else:
    io.print("C")
end
```

**Enum variants** in `case` arms are written **without** a leading dot (S3):

```ori
match direction
case North:
    move_north()
case South:
    move_south()
case East:
    move_east()
case West:
    move_west()
end
```

Leading-dot patterns such as `case North:` are rejected with
`parse.case_dot_variant_removed`. Enum *literals* outside match may still use
other forms (see later chapters / S3 block 4).

---

## `using` — Resource Cleanup

```ori
using file: ori.fs.File = try ori.fs.open_read(path)
const content: string = try ori.fs.read_all(file)
return ok(content)
```

`using` binds a resource and guarantees its cleanup function is called when
the binding goes out of scope, regardless of how the scope exits:

- Normal `end` of block
- `return`
- `try` propagation
- `break` or `continue`
- Panic

Multiple `using` bindings are cleaned up in **reverse declaration order** (LIFO).

The cleanup function for a type is defined by implementing `Disposable`:

```ori
trait Disposable
    mut dispose()
end
```

---

## `check` — Programmer Assertion

```ori
check count >= 0
check index < len(items), "index out of expected range"
```

`check` asserts a condition that must hold as a programmer invariant.
A failed `check` is a non-recoverable panic — it is not meant for business
logic error handling.

The optional second argument is a message string for the panic message.

Use `result[T, E]` for expected, recoverable failures.

**Difference from `if` value contracts:**
`check` is imperative — it is a statement called explicitly at a point
in the code. `if` contracts on fields and parameters are declarative —
they are checked automatically at construction or call time.

---

## `break` and `continue`

```ori
loop
    if done
        break           -- exit the loop
    end
    if skip_this
        continue        -- skip to next iteration
    end
    process()
end
```

`break` and `continue` apply to the innermost enclosing loop.
They are not valid outside of a loop body.

---

## Expression Statements

Any expression may appear as a statement. The value is discarded.
The most common use is a function call with side effects:

```ori
io.print("hello")
counter.increment()
list.push(item)
```

If a function returns `result[T, E]` and the result is discarded without
propagation or handling, the compiler emits a warning: `unused result`.

---

## Block

A block is a sequence of statements terminated by `end`.
Blocks do not produce values (they are not expressions).

```ori
-- Inside a function:
const x: int = 1
const y: int = 2
return x + y
end
```

Nested scopes are created by `if`, `while`, `for`, `loop`, `match`, and
function bodies. Each nested scope can declare bindings that shadow outer
bindings — but shadowing within the same scope level is a compile error.
