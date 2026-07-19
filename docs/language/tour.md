# Ori language tour (S3)

> **Audience:** people learning to read and write Ori  
> **Portuguese:** [tour.pt-BR.md](tour.pt-BR.md)  
> **Normative detail:** [../spec/01-overview.md](../spec/01-overview.md)  
> **Surface:** S3 `0.3.0` + inference B `0.3.1` · result ctors `ok`/`err`

This tour matches **what the compiler accepts today**. Pre-S3 forms are hard
errors (`ori migrate-syntax` rewrites many of them).

---

## 1. A complete small program

```ori
module app.hello

import ori.io = io

main()
    io.println("Hello, Ori!")
    const answer: int = 21 * 2
    io.println(f"The answer is {answer}")
end
```

```bash
ori run main.orl
```

| Idea | Form |
|------|------|
| File belongs to a namespace | `module app.hello` first line |
| Import with short name | `import ori.io = io` (path **left**, alias **right**) |
| Entry point | `main()` — no `func` keyword |
| Blocks | end with `end` |
| Types | explicit on bindings when public / when not inferred |

---

## 2. Modules and imports

Three import forms:

```ori
import ori.fs (read_text, write_text)   -- selective
import ori.string = str                 -- alias
import ori.math                         -- whole module: only ori.math.…
```

- Bare `import ori.io` does **not** create a local name `io`.
- Prefer parent modules: `ori.fs`, `ori.string` (not `ori.fs.utils` in new code).
- Domain type aliases exist on some parents, e.g. `import ori.fs (TextResult)`.

---

## 3. Types you will use every day

| Type | Meaning |
|------|---------|
| `int`, `float`, `bool`, `string`, `bytes` | primitives / text / binary |
| `list[T]`, `map[K, V]`, `set[T]` | collections |
| `optional[T]` | presence or `none` (not null) |
| `result[T, E]` | success `ok(T)` or failure `err(E)` |
| `void` | no useful value (side effects) |

Composite types always use **`[]`**, never `<>` or `list of T`.

```ori
const names: list[string] = ["ada", "grace"]
const maybe: optional[int] = none
const file: result[string, string] = err("missing")
```

### Local inference (option B)

You may omit the type on a **local** `const`/`var` when the right-hand side is:

- a field access, index, call with known return type, or pipe `|>`

```ori
const n = ori.list.len(names)     -- call
const first = names[0]            -- index
const upper = "hi" |> str.to_upper  -- pipe (if str imported)
```

You still annotate public API and cases the checker cannot decide.

---

## 4. Control flow

```ori
if n > 0
    io.println("positive")
elif n == 0
    io.println("zero")
else
    io.println("negative")
end

while i < 10
    i = i + 1
end

for item in names
    io.println(item)
end

match value
    case ok(text):
        io.println(text)
    case err(msg):
        io.eprintln(msg)
end

match score
    case n if n >= 90:
        io.println("A")
    case n if n >= 80:
        io.println("B")
    case else:
        io.println("C")
end

-- `match` also works as an expression: each arm is a single value
const grade: string = match score
    case n if n >= 90: "A"
    case else: "C"
end
```

- Use **`elif`**, not `else if`.
- Enum patterns: `case Variant` / `case Variant(fields)` — no leading `.`.
- `case pattern if cond:` guards an arm: a false guard falls through to the
  next case. `case else:` is the explicit fallback (guarded cases do not
  count toward exhaustiveness).
- `case a or b:` groups alternatives (the word `or`, not `|`). Alternatives
  cannot bind values, and together they count as full coverage.

---

## 5. Results and optionals (no exceptions as control flow)

```ori
load(path: string) -> result[string, string]
    return ori.fs.read_text(path)
end

main() -> result[void, string]
    const text: string = try load("notes.txt")
    io.println(text)
    return ok()
end
```

| Construct | Role |
|-----------|------|
| `ok(v)` / `err(e)` | build a `result` |
| `some(v)` / `none` | build an `optional` |
| `try expr` | propagate `err` or `none` |
| `match` | exhaustive handling |
| `if some(x) = expr` | branch on presence, binding the value |
| `if ok(v) = expr` / `if err(e) = expr` | branch on a `result`, binding either side |

```ori
if some(user) = find_user(id)
    greet(user)
else
    io.println("not found")
end

if ok(value) = divide(10, 2)
    io.println(string(value))
end

if err(message) = divide(1, 0)
    io.println(message)   -- taken when the result is NOT ok
end
```

Postfix `expr?` was **removed** in S3.

---

## 6. Structs, enums, traits

```ori
struct Point
    x: int
    y: int
end

enum Color
    Red
    Green
    Rgb(r: int, g: int, b: int)
end

const p: Point = Point { x: 1, y: 2 }
const c: Color = Color.Rgb(r: 1, g: 2, b: 3)

-- update expression: a new value derived from `p`; `p` is untouched
const moved: Point = p with { x: 10 } end
```

Traits use **`apply Type`** + **`use Trait`** (not `implement Trait for Type`).
Import the trait module first (`import ori.core = core`).

```ori
import ori.core = core
import ori.io = io

struct Point
    x: int
    y: int
end

apply Point
    use core.Displayable
        display(self) -> string
            return f"({self.x}, {self.y})"
        end
    end
end

main()
    const p: Point = Point { x: 1, y: 2 }
    io.println(string(p))   -- Displayable via string(value)
end
```

---

## 7. Functions and style

```ori
add(a: int, b: int) -> int
    return a + b
end

greet(name: string) -> void
    io.println("hi " + name)
end

-- expression body
double(n: int) -> int => n * 2
```

- Closures: `(x: int) => x + 1` or `(x: int) … end`.
- Pipe: `value |> pure_fn` is typed as `pure_fn(value)`.
- One-argument poetic call on the same line is allowed for readability; nested
  poetic calls are rejected.

---

## 8. Projects

```text
my_app/
  ori.proj      -- required
  main.orl      -- recommended entry
  docs/         -- optional .oridoc sidecars
```

```bash
ori new my_app
cd my_app
ori check main.orl
ori run main.orl
ori test main.orl
```

See [First project](../guides/first-project.md) and
[spec/17-project-and-docs.md](../spec/17-project-and-docs.md).

---

## 9. What not to write (pre-S3)

| Avoid | Use instead |
|-------|-------------|
| `namespace` | `module` |
| `func name()` | `name()` |
| `import x as y` / `only` | `import path = y` / `(…)` |
| `list of T` / `Foo<T>` | `list[T]` / `Foo[T]` |
| `success` / `error` | `ok` / `err` |
| `else if` | `elif` |
| `expr?` | `try expr` |
| `implement Trait for T` | `apply T` + `use Trait` |

```bash
ori migrate-syntax path/to/sources
```

---

## 10. Async (native)

```ori
module app.main

import ori.io = io
import ori.task = task

async main()
    await task.sleep(10)
    io.println("done")
end
```

- `async main()` is the async entry point; use `await` only inside `async` functions.
- File/net await helpers: `fs.read_text_async`, `net.connect_async`, …
- Example: [`examples/async_demo`](../../examples/async_demo/).

C/debug backend **rejects** async (native is the reference).

---

## 11. Where to go next

| Goal | Doc |
|------|-----|
| Install package (Linux primary) | [../install.md](../install.md) |
| Recipes | [../guides/cookbook.md](../guides/cookbook.md) |
| Errors mental model | [../guides/errors-null-void.md](../guides/errors-null-void.md) |
| Runnable samples | [../../examples/](../../examples/) |
| Full grammar / types | [../spec/](../spec/README.md) |
| ABI / runtime | [../spec/19-abi.md](../spec/19-abi.md) |
