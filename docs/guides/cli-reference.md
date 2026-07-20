# CLI reference

> **Audience:** anyone using the `ori` command  
> **Portuguese:** [cli-reference.pt-BR.md](cli-reference.pt-BR.md)  
> Generated from `ori --help` at `0.3.x`. Run `ori <command> --help` for the
> full flag list of any command.

Check what you have installed:

```bash
ori --version
```

---

## The five you need first

| Command | What it does |
|---|---|
| `ori new <path>` | Create a new project (`--lib` for a library) |
| `ori check <file>` | Type-check and report diagnostics — no binary produced |
| `ori run <file>` | Compile and run through the native runtime |
| `ori test <file>` | Run functions marked `@test` |
| `ori explain <code>` | Explain a diagnostic code, e.g. `ori explain type.type_mismatch` |

```bash
ori new my_app
cd my_app
ori check main.orl
ori run main.orl
```

`ori check .` walks up until it finds `ori.proj`, so it works from any
subfolder of a project. `ori check ori.proj` is equivalent.

---

## Building

| Command | What it does |
|---|---|
| `ori compile <file> -o <out>` | Compile to a native binary via Cranelift and the packaged runtime |
| `ori build` | Build a file or project through the native backend |
| `ori run <file>` | Compile and run in one step |

```bash
ori compile main.orl -o my_app
./my_app
```

Compiling a shared library instead of an executable:

```bash
ori compile main.orl --lib -o libmy_app.so
```

Only functions marked `@c_export` are visible to C. The export surface covers
scalars (`int`, `float`, `bool`, …) and **`string`**, which crosses as a
NUL-terminated `const char *`. Aggregates — structs, `list`, `map`, `optional`,
`result` — are not exportable yet.

A string **returned** to the host is owned by the host: free it with
`ori_arc_release`, or it leaks. A string **passed in** stays owned by the host;
Ori never frees it. See [../spec/19-abi.md](../spec/19-abi.md) §8.3b.

---

## Project and packages

| Command | What it does |
|---|---|
| `ori new <path>` | Create a project in a new directory |
| `ori init` | Initialize a project in an existing directory |
| `ori summary` | Print entry, namespaces, and imports of the project |
| `ori install <name> --path .` | Install a package into the local cache |
| `ori get` | Fetch git/path dependencies into the local cache |
| `ori publish` | Publish a package to the registry in `ORI_REGISTRY` |
| `ori update` | Update the toolchain to the latest published release |

Manifest fields are specified in
[../spec/17-project-and-docs.md](../spec/17-project-and-docs.md).

---

## Documentation

| Command | What it does |
|---|---|
| `ori doc file <file>` | Extract doc comments as Markdown or HTML |
| `ori doc check` | Validate inline docs and `.oridoc` sidecars |
| `ori doc export` | Export stdlib + error catalog JSON for the website |

---

## Formatting and migration

| Command | What it does |
|---|---|
| `ori fmt <file>` | Format a source file and print the result |
| `ori migrate-syntax <path>` | Best-effort rewrite of pre-S3 syntax in `.orl` files |

`ori migrate-syntax` handles the mechanical S3 cutover. It rewrites:

| From | To |
|---|---|
| `namespace` | `module` |
| `import x as y` / `import x only (…)` | `import x = y` / `import x (…)` |
| `implement T for X` | `apply X` + `use T` (header only) |
| `apply Trait to Type` | `apply Type` + `use Trait` |
| nested single-`use` block | compact `apply T use Trait` header |
| `type Name = …` | `alias Name = …` |
| `Foo<T>` / `list of T` | `Foo[T]` / `list[T]` |
| `where T is Trait` | `for T: Trait` |
| `success` / `error` | `ok` / `err` |
| `else if` | `elif` |
| `do(x) =>` | `(x) =>` |
| `case .Variant` | `case Variant` |
| declaration `func` | stripped |

Two things it will **not** finish for you, and reports as notes instead:

- postfix `expr?` is not auto-migrated — rewrite it to `try expr` by hand;
- a rewritten `implement` header leaves the body to review as a `use` section.

Use `--dry-run` to preview. Re-run `ori check` afterwards either way.

---

## Environment and diagnosis

| Command | What it does |
|---|---|
| `ori doctor` | Report environment, stdlib, and native runtime health |
| `ori explain <code>` | Explain a diagnostic code from the error catalog |

Run `ori doctor` first whenever a build fails for a reason that is not in your
source.

---

## Interactive

| Command | What it does |
|---|---|
| `ori repl` | Small interactive REPL backed by the native JIT |

The REPL is explicitly an **experimental** surface — its limits may change
before `1.0` ([../spec/18-stability-and-compatibility.md](../spec/18-stability-and-compatibility.md)).

---

## Compiler debugging

These print compiler intermediates. They are for working **on** Ori, not with
it.

| Command | What it does |
|---|---|
| `ori lex <file>` | Print the raw token stream |
| `ori parse <file>` | Print the AST |
| `ori emit c <file>` | Emit C source through the partial debug backend |

The C backend is a debug aid, not a semantic reference — the native backend is
([../spec/14-backend-support.md](../spec/14-backend-support.md)).

---

## Environment variables

| Variable | Effect |
|---|---|
| `ORI_PACKAGE_CACHE` | Where packages are installed (default `~/.ori/packages`) |
| `ORI_REGISTRY` | Registry URL used by `ori publish` / `ori install` |
| `ORI_REGISTRY_TOKEN` | Auth token for the registry |
| `ORI_STDLIB_ROOT` | Override the stdlib location |
| `ORI_RUNTIME_LIB` / `ORI_RUNTIME_CDYLIB` | Point at a specific staged native runtime |
| `ORI_REQUIRE_PACKAGED_RUNTIME` | Fail instead of falling back to a Cargo build of the runtime |
| `ORI_USE_JIT` / `ORI_USE_AOT` | Force the execution route for `ori run` |
| `ORI_USE_SYSTEM_LINKER` / `ORI_USE_BUNDLED_RUST_LLD` | Choose the linker |

`--no-color` is accepted by every command and disables ANSI output.
