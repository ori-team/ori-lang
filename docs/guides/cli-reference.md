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
| `ori test <file> [--doc]` | Run functions marked `@test` (or `--doc` to run markdown doctests) |
| `ori explain <code>` | Explain a diagnostic code, e.g. `ori explain type.type_mismatch` |

```bash
ori new my_app
cd my_app
ori check main.orl
ori run main.orl
```

`ori check .` walks up until it finds `ori.proj`, so it works from any
subfolder of a project. `ori check ori.proj` is equivalent.

All compilation commands accept the same conditional-compilation selection:

```bash
ori check . --features tls,telemetry
ori run . --no-default-features
ori check . --execution-profile embedded
ori check . --target x86_64-unknown-linux-gnu
```

Features must be declared under `[features]` in the project/package manifest.
These flags also affect AOT cache fingerprints. `--target` selects cfg facts
and native runtime artifacts; it does not by itself promise full native
cross-compilation. A triple whose architecture or OS is outside cfg v1 is
rejected instead of being treated as an OS-free target.
Likewise, `--execution-profile embedded` selects cfg branches; it does not yet
turn the desktop runtime into a freestanding or sandboxed runtime.

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

Only functions marked `@c_export` are visible to C. The ABI currently covers
scalars, `string`, non-empty non-generic scalar structs, managed structs through
opaque ARC handles, and direct `optional`/`result` bridges. Direct `list`,
`map`, `set`, `tuple`, nested sum types, generic structs, and empty structs are
still rejected. The generated header is the canonical host declaration; see
[ABI-1](../spec/19-abi.md#83b-c_export--the-host-facing-surface).

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
| `ori lock [path]` | Resolve dependencies and atomically write digest-bearing `ori.lock` v2 |
| `ori lock [path] --locked --offline` | Restore the exact lock from verified cache/path content without network |
| `ori publish` | Immutably publish a package and SHA-256 archive digest to `ORI_REGISTRY` |
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

## Formatting, linting, and tools

| Command | What it does |
|---|---|
| `ori fmt <path> [-w / --write] [-c / --check]` | Format a source file or recursively format directory (`-w` in-place, `-c` check) |
| `ori lint <path>` | Run semantic code linter for unused variables and code redundancy |
| `ori daemon [--stdio]` | Run the experimental process-persistent stdio prototype; it rebuilds fresh pipelines and is not yet a complete/cached JSON-RPC service |
| `ori bindgen <header.h> [--module <name>]` | Generate low-level Ori `extern "c"` bindings and `@repr("C")` structs from C header |
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

Maintainers can compile and run a hostile `check` message through the generated
C under AddressSanitizer and UndefinedBehaviorSanitizer:

```sh
cd compiler
cargo test -p ori-driver --test c_backend_sanitizers -- --nocapture
```

The test probes `clang` and then `cc`. It prints an explicit `SKIP` when neither
compiler can compile and run with both sanitizers. Set `ORI_C_SANITIZER_CC` to
one compiler executable, or set `ORI_REQUIRE_C_SANITIZERS=1` to turn missing
sanitizer support into a gate failure (recommended in CI).

## Program debugging

Use the cooperative native debugger for native programs, including async
functions and closures:

```text
ori debug examples/cli_args/main.orl --breakpoint 41
```

At a stop, `c` continues, `s` steps to the next instrumented line, and `q`
terminates the target. The terminal adapter displays the current source
location, the current stack (including async frames), and visible locals. The
same variable catalogue is generated on every desktop platform; live values
come from the cooperative runtime snapshot.

For an IDE, start the minimal Debug Adapter Protocol server over stdio:

```text
ori debug --dap
```

The DAP adapter accepts `initialize`, `launch`, `setBreakpoints`,
`configurationDone`, `continue`, `next`, `threads`, `stackTrace`, `scopes`,
`variables`, `evaluate`, and `disconnect`. Struct fields, `optional`, `result`,
enum payloads, maps, sets, and supported opaque collections are listed with
qualified names (for example `user.name`), lists expose `length`/`capacity` and
bounded indexed children, async frames remain visible across `await`, and
closure captures appear in the closure frame. Managed strings and bytes show a
bounded preview (bytes use hexadecimal); static or foreign buffers are read
only after an exact length is registered. `evaluate` is deliberately limited
to scalar arithmetic, comparisons, boolean logic, and strings from the latest
stopped snapshot; it never executes target code. Native builds also write
`program.debug.json`, a portable catalogue of parameters, locals, pattern
bindings and closure captures with source lines.

---

## Environment variables

| Variable | Effect |
|---|---|
| `ORI_PACKAGE_CACHE` | Where packages are installed (default `~/.ori/packages`) |
| `ORI_REGISTRY` | Registry path or HTTPS URL used by `ori publish` / `ori install` |
| `ORI_REGISTRY_TOKEN` | Auth token for the registry |
| `ORI_OFFLINE` | Refuse package network access and require verified cache entries |
| `ORI_ALLOW_INSECURE_REGISTRY` | Allow plain HTTP only for an explicitly trusted local-development registry |
| `ORI_STDLIB_ROOT` | Override the stdlib location |
| `ORI_RUNTIME_LIB` / `ORI_RUNTIME_CDYLIB` | Point at a specific staged native runtime |
| `ORI_REQUIRE_PACKAGED_RUNTIME` | Fail instead of falling back to a Cargo build of the runtime |
| `ORI_USE_JIT` / `ORI_USE_AOT` | Force the execution route for `ori run` |
| `ORI_USE_SYSTEM_LINKER` / `ORI_USE_BUNDLED_RUST_LLD` | Choose the linker |
| `ORI_DISABLE_INCREMENTAL` | Disable matching-output and per-file object reuse from `.ori/incremental.json` / `.ori/modules/` |
| `ORI_OBJCOPY` | Select `objcopy`/`llvm-objcopy` for Linux DWARF section emission |

`--no-color` is accepted by every command and disables ANSI output.

When a project contains `ori.lock`, dependency resolution is checked before
compilation. The lock pins normalized source identity, exact Git revision, and
the SHA-256 of every dependency tree; changed bytes fail instead of being
silently re-resolved. Registry HTTPS is mandatory by default, published
versions are immutable, and archive downloads/extraction have hard byte, entry,
path, and depth limits. Imports remain package-scoped: use a dependency's qualified module
name (`demo.math`) rather than relying on a bare module search across packages.
Native rebuilds report the number of changed source modules. Rebuilds keep
unchanged source objects in `.ori/modules/` and link them with regenerated
objects; projects with dynamic global initialization, `--lib`, or explicit
debug instrumentation use the conservative monolithic route. On Windows, a
successful native link writes the deterministic sibling `.pdb` when the
linker supports it.
