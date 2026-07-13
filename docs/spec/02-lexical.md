# Ori Language Specification — Chapter 02: Lexical Structure

> Status: normative
> Audience: compiler implementers

---

## Source Encoding

Ori source files are UTF-8 encoded. The `.orl` extension is canonical.

A byte order mark (BOM) at the start of a file is accepted and ignored.
A BOM anywhere else is a lexical error (`lex.unexpected_character`).

---

## Whitespace

Spaces, tabs, and newlines are whitespace. Whitespace separates tokens and is
otherwise not significant. Ori does not use indentation as syntax.

Recommended indentation: 4 spaces.

---

## Comments

### Line Comments

```lua
-- this is a line comment
```

A `--` sequence begins a line comment. Everything from `--` to the end of the
line is ignored by the compiler.

### Block Comments

```lua
--|
This is a block comment.
It spans multiple lines.
|--
```

`--|` opens a block comment. `|--` closes it. Block comments do not nest.
The tokens `--|` and `|--` are mirrored by design — they cannot appear inside
normal code because `|--` is not valid syntax anywhere else.
An unclosed block comment is a lexical error (`lex.unclosed_block_comment`).

### Documentation Comments

Current implementation status:

- The lexer emits `--| ... |--` as `BlockComment` trivia.
- The parser skips this trivia for normal compilation.
- `ori doc file <file>` extracts immediately leading documentation comments as
  Markdown.
- `@param` tags are validated against documented function parameters and emit
  `doc.param_name_mismatch` when a tag names a missing parameter.
- Long-form documentation can live in `.oridoc`; see
  `docs/spec/17-project-and-docs.md`.

A block comment placed immediately before a declaration is treated as a
**documentation comment** for `ori doc`:

```lua
--|
Calculates the area of a rectangle.
@param width  Width in pixels (must be positive).
@param height Height in pixels (must be positive).
@returns The computed area.
@example
    const a: int = area(10, 5)  -- 50
|--
pub func area(width: int, height: int) -> int
    return width * height
end
```

Recognized doc comment tags:

| Tag | Description |
|---|---|
| `@param name description` | Documents a parameter |
| `@returns description` | Documents the return value |

Other tags are preserved as plain text until richer documentation tooling is
implemented.

---

## Reserved Words

The following identifiers are reserved and cannot be used as user-defined names:

```
namespace  import     as         pub        public
func       return     end        const      var
if         else       while      for        in
repeat     break      continue
match      case       loop
struct     trait      apply      implement  enum
where      is         alias      do
and        or         not
true       false      none       success    error    some
mut        self       attr       extern
any        optional   result     list       map      set
range      void
using      check      with       then       tuple    lazy
```

### Canonical keywords (use these)

| Role | Canonical | Deprecated alias (emits warning) |
|------|-----------|----------------------------------|
| Visibility | **`pub`** | `public` → `parse.deprecated_public` |
| Trait attachment | **`apply Trait to Type`** | `implement Trait for Type` → `parse.deprecated_implement` |

New code, examples, and documentation **must** use `pub` and `apply`. The
deprecated spellings remain in the reserved-word list only so the parser can
accept and warn on existing sources until they are migrated.

Note: `times` was removed from the reserved list. See Contextual Keywords below.

### Contextual Keywords

The following words are recognized only in specific syntactic positions and may
be used as identifiers elsewhere:

| Word | Position |
|---|---|
| `only` | After `import module`, starts a selective import list |
| `c` | After `extern`, names the C ABI: `extern c` |
| `host` | After `extern`, names the host ABI |
| `it` | Inside an `if` value contract on a field or parameter — refers to the value being checked |
| `times` | After `repeat expression` — optional readability word: `repeat 5 times` |
| `try` | Before an expression — readable propagation form: `try read_config(path)` |
| `to` | After the trait name in `apply Trait to Type`, or in `map of K to V` |
| `of` | After a single-arg type constructor: `list of string`; also starts `map of` |
| `by` | After a range `start..end by step` |
| `imports` | Starts an `imports … end` import batch block |

---

## Identifiers

An identifier starts with a Unicode letter or `_`, followed by zero or more
Unicode letters, digits, or `_`.

```
identifier = letter { letter | digit | "_" } ;
letter     = any Unicode letter or "_" ;
digit      = "0" | "1" | ... | "9" ;
```

Conventions (enforced by `ori fmt`):

| Shape | Convention |
|---|---|
| Types, traits, enums | `PascalCase` |
| Functions, variables, namespaces | `snake_case` |
| Constants at module level | `SCREAMING_SNAKE_CASE` (optional) |

Shadowing of an existing binding in the same scope is a compile error.

---

## Literals

### Boolean

```lua
true
false
```

### Integer

```lua
0
42
1_000_000       -- underscores allowed as separators
0xFF            -- hexadecimal
0b1010_1010     -- binary
0o755           -- octal
```

The default integer type is `int` (64-bit signed). Explicit suffixes select a
specific width:

```lua
42i8    42i16    42i32    42i64
42u8    42u16    42u32    42u64
```

### Float

```lua
3.14
1.0e10
1.0e-5
6.022_140_76e23
```

The default float type is `float` (64-bit). Explicit suffix:

```lua
3.14f32    3.14f64
```

### String

Single-line strings are delimited by double quotes:

```lua
"hello"
"line one\nline two"
"tab\there"
"unicode \u{1F600}"
```

Escape sequences:

| Sequence | Meaning |
|---|---|
| `\\` | Backslash |
| `\"` | Double quote |
| `\n` | Newline |
| `\r` | Carriage return |
| `\t` | Tab |
| `\0` | Null byte |
| `\u{XXXX}` | Unicode scalar value |

### Multi-line Strings (Triple-quote)

```lua
const sql: string = """
    SELECT *
    FROM users
    WHERE active = true
    """
```

Rules:

- The opening `"""` must be followed by a newline.
- The closing `"""` sets the indentation baseline: all leading whitespace
  up to the column of `"""` is stripped from every line.
- The closing `"""` line is not included in the string content.
- Escape sequences are processed normally inside triple-quote strings.

### Interpolated Strings

```lua
const name: string = "Ada"
const greeting: string = f"hello {name}"
const detail: string = f"score: {score + 1}"
```

Rules:

- Prefix `f` activates interpolation.
- `{expr}` interpolates any expression whose type implements `Displayable`.
- Interpolation expressions may not contain string literals.
- Multi-line interpolated strings: prefix `f` on a triple-quote: `f"""..."""`.

### Byte Strings

```lua
const raw: bytes = b"hello"
const hex: bytes = b"\xFF\x00"
```

Prefix `b` produces a `bytes` literal. No Unicode escapes in `b"..."`.

### Range Literals

```lua
0..9        -- range<int>: 0, 1, 2, ..., 9  (inclusive both ends)
5..3        -- range<int>: 5, 4, 3          (descending, inclusive)
```

Ranges are always inclusive on both ends. Direction is determined by whether
`start <= end` (ascending) or `start > end` (descending). Equal endpoints
yield a range of exactly one element. Current ranges use `int` endpoints only.

---

## Operators and Punctuation

```
.   ..  ->  =>
+   -   *   /   %
==  !=  <   <=  >   >=
=
?   |>
(   )   [   ]   <   >
:   ,
@
--|   |--
```

The `..` token is the range operator and slice operator.
The `->` token separates parameter lists from return types.
The `=>` token separates closure parameters from expression bodies.
The `?` token is the compact propagation operator. The readable form is the
contextual keyword `try` before an expression.
The `|>` token is the pipe operator.
The `@` token is the attribute prefix: `@test`, `@deprecated("message")`.
The `--|` / `|--` tokens delimit block and documentation comments.

### Tuple Field Access

Fields of a `tuple<...>` are accessed by integer index after `.`:

```lua
const pair: tuple<int, string> = tuple(1, "one")
const n: int    = pair.0
const s: string = pair.1
```

The lexer accepts `INTEGER` tokens immediately after `.` in this context.
The indices are zero-based and must be valid compile-time integer literals.

### Attribute Annotations

Current implementation status:

- Attribute syntax is parsed on top-level declarations and stored in the AST.
- Built-in attribute names, targets, duplicate uses, and argument shapes are validated.
- `@deprecated("message")` emits `attr.deprecated` warnings at use sites.
- `@test` marks concrete no-arg/no-return test functions for `ori test`.
- `@inline`, `@no_inline`, and `@cfg` are validated but not acted on by the compiler yet.
- Unknown attributes are rejected with `attr.unknown`.
- The emitted attribute diagnostics are listed in `docs/spec/13-error-catalog.md`.

Attributes are reserved for declaration metadata:

```lua
@test
func test_addition()
    check 1 + 1 == 2
end

@deprecated("use new_api() instead")
pub func old_api() -> int

@inline
func hot_path(n: int) -> int
    return n * 2
end
```

Built-in attributes:

| Attribute | Applies to | Current validation | Planned effect |
|---|---|---|---|
| `@test` | `func` | no arguments; function must have no type parameters, no value parameters, and no return value | Runs through `ori test` |
| `@deprecated("msg")` | any declaration | exactly one string argument | Emits `attr.deprecated` warning at use sites |
| `@inline` | `func` | no arguments | Hint to inline at call sites |
| `@no_inline` | `func` | no arguments | Prohibit inlining |
| `@cfg("condition")` or `@cfg(key: value)` | any declaration | exactly one string or named argument | Conditionally include based on build config |

Custom attributes are not part of the planned v1 contract. They are rejected
with `attr.unknown`.

---

## Operator Precedence

Operators bind in the following order (highest to lowest):

| Level | Operators | Associativity |
|---|---|---|
| 1 | `.field`  `call()`  `[index]` | Left |
| 2 | `?` | Postfix |
| 3 | `-` (unary)  `not`  `try` | Prefix |
| 4 | `*`  `/`  `%` | Left |
| 5 | `+`  `-` | Left |
| 6 | `==`  `!=`  `<`  `<=`  `>`  `>=` | Non-chainable* |
| 7 | `and` | Left |
| 8 | `or` | Left |
| 9 | `\|>` | Left |

*Comparison operators do not chain. `a < b < c` is a compile error.
Use `a < b and b < c` instead.

---

## Token Summary

| Category | Examples |
|---|---|
| Keywords | `func`, `struct`, `namespace`, `pub`, `apply`, `loop`, `do` ... |
| Identifiers | `player`, `User`, `get_name`, `_internal` |
| Integer literals | `0`, `42`, `0xFF`, `1_000` |
| Float literals | `3.14`, `1.0e10` |
| String literals | `"hello"`, `f"hi {name}"`, `"""..."""` |
| Byte literals | `b"data"` |
| Boolean literals | `true`, `false` |
| Range literals | `0..9`, `5..3` |
| Operators | `+`, `-`, `==`, `?`, `\|>`, `..` |
| Delimiters | `(`, `)`, `[`, `]`, `:`, `,` |
| Comments | `-- line`, `--- block ---` |
