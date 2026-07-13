# Ori Language Specification — Chapter 06: Statements and Control Flow

> Status: normative
> Audience: compiler implementers

---

## Overview

Statements produce effects. They appear inside function bodies (blocks).
A block is a sequence of statements closed by `end`.

---

## Binding Declarations

### `const`

Declares an immutable binding.

```lua
const name: string = "Ada"
const max: int = 100
```

- Cannot be reassigned after declaration.
- The type annotation is required.
- The initializer is evaluated once, at declaration time.

### `var`

Declares a mutable binding.

```lua
var count: int = 0
count = count + 1
```

- Can be reassigned with `=`.
- The type annotation is required.
- Compound assignment operators are available: `+=`, `-=`, `*=`, `/=`.

---

## Assignment

```lua
count = 42
user.name = "Bo"
items[0] = 99
```

The left-hand side must be a mutable binding or a mutable path through a `var`.
Assigning to a `const` binding is a compile error.

---

## Return

```lua
return value
return          -- implicit void
```

`return` exits the enclosing function. If the function declares a return type,
the returned expression must match.

A function body that reaches the end without `return` is a compile error if
the return type is not `void`.

---

## `if` / `else`

```lua
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
else if score >= 80
    io.print("B")
else
    io.print("C")
end
```

The condition must be `bool`. There is no truthy/falsy coercion.

---

## `if some` — Optional Binding

```lua
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

## `while`

```lua
while count < 10
    count += 1
end
```

The condition must be `bool`. Evaluated before each iteration.

---

## `while some` — Optional Loop

```lua
while some(line) = reader.next_line()
    process(line)
end
```

Continues as long as `next_line()` returns `some(v)`. Stops on `none`.
`line` is scoped to each iteration of the loop body.

---

## `loop` — Infinite Loop

```lua
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

```lua
for item in items
    process(item)
end
```

Iterates over built-in iterables or a value that implements `core.Iterable`.

Built-in iterables: `list<T>`, `map<K, V>`, `set<T>`, `string`, `bytes`,
and `range<int>`.

Custom iterable contract:

- import the core trait namespace, usually `import ori.core as core`;
- `apply core.Iterable to` the concrete type;
- provide `mut func next() -> optional<T>`;
- the `for` binding has type `T`;
- the second binding is the zero-based `int` index.

**With index:**

```lua
for item, index in items
    io.print(f"{index}: {item}")
end
```

For `list<T>`, `set<T>`, `string`, `bytes`, ranges, and custom iterables:
second binding is the `int` index.
For `map<K, V>`: second binding is the value `V` (first is the key `K`).

**Range iteration:**

```lua
for i in 0..9
    io.print(string(i))
end

for i in 9..0          -- descending: 9, 8, ..., 0
    io.print(string(i))
end
```

---

## `repeat` — Fixed Count

```lua
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

```lua
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
```lua
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

**Guard:**

```lua
match score
case n if n >= 90:
    io.print("A")
case n if n >= 80:
    io.print("B")
case else:
    io.print("C")
end
```

**Shorthand enum variant** (when enum type is known):

```lua
match direction
case .North:
    move_north()
case .South:
    move_south()
case .East:
    move_east()
case .West:
    move_west()
end
```

---

## `using` — Resource Cleanup

```lua
using file: ori.fs.File = try ori.fs.open_read(path)
const content: string = try ori.fs.read_all(file)
return success(content)
```

`using` binds a resource and guarantees its cleanup function is called when
the binding goes out of scope, regardless of how the scope exits:

- Normal `end` of block
- `return`
- `try`/`?` propagation
- `break` or `continue`
- Panic

Multiple `using` bindings are cleaned up in **reverse declaration order** (LIFO).

The cleanup function for a type is defined by applying `Disposable` with
`apply Disposable to Type`:

```lua
trait Disposable
    mut func dispose()
end

apply Disposable to File
    mut func dispose()
        self.close()
    end
end
```

---

## `check` — Programmer Assertion

```lua
check count >= 0
check index < len(items), "index out of expected range"
```

`check` asserts a condition that must hold as a programmer invariant.
A failed `check` is a non-recoverable panic — it is not meant for business
logic error handling.

The optional second argument is a message string for the panic message.

Use `result<T, E>` for expected, recoverable failures.

**Difference from `if` value contracts:**
`check` is imperative — it is a statement called explicitly at a point
in the code. `if` contracts on fields and parameters are declarative —
they are checked automatically at construction or call time.

---

## `break` and `continue`

```lua
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

```lua
io.print("hello")
counter.increment()
list.push(item)
```

If a function returns `result<T, E>` and the result is discarded without
propagation or handling, the compiler emits a warning: `unused result`.

---

## Block

A block is a sequence of statements terminated by `end`.
Blocks do not produce values (they are not expressions).

```lua
-- Inside a function:
const x: int = 1
const y: int = 2
return x + y
end
```

Nested scopes are created by `if`, `while`, `for`, `loop`, `match`, and
function bodies. Each nested scope can declare bindings that shadow outer
bindings — but shadowing within the same scope level is a compile error.
