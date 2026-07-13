# Ori

Ori is a reading-first, explicitly typed programming language compiled to native
code. Its compiler is written in Rust and is built around a direct goal: make
programs easier to read, inspect, diagnose, and maintain.

Ori is pre-1.0. It is useful for compiler, language-design, tooling, and
runtime work, but the language is still allowed to change before a stable 1.0
contract.

**Languages:** English | [Portuguese](README.pt-BR.md) | [Japanese](README.ja.md)

**Project menu:** [Specification](docs/spec/README.md) | [Planning](docs/planning/README.md) | [First project](docs/guides/first-project-and-packages.md) | [Cookbook](docs/guides/cookbook-pequeno-medio.md) | [Bug reports](docs/guides/reportar-bugs.md) | [Standard library](stdlib/README.md) | [Runtime](runtime/README.md) | [Examples](examples/) | [Changelog](CHANGELOG.md) | [Contributing](CONTRIBUTING.md)

## Contents

- [What Ori is](#what-ori-is)
- [Why Ori exists](#why-ori-exists)
- [Current status](#current-status)
- [Quick start](#quick-start)
- [A first program](#a-first-program)
- [CLI overview](#cli-overview)
- [Project docs](#project-docs)
- [Language overview](#language-overview)
- [Compiler architecture](#compiler-architecture)
- [Standard library](#standard-library)
- [Editor tooling](#editor-tooling)
- [Repository layout](#repository-layout)
- [Development workflow](#development-workflow)
- [Release layout](#release-layout)
- [Known limitations](#known-limitations)
- [Roadmap](#roadmap)
- [License](#license)

## What Ori is

Ori is a statically typed language with explicit module namespaces (namespace),
explicit types, absence/failure types (optional, result), structured errors,
deterministic cleanup, and native code generation.

The current compiler pipeline is:

```text
.orl source
  -> lexer
  -> parser
  -> name resolver
  -> type checker
  -> HIR
  -> Cranelift native backend
  -> runtime-linked binary or JIT execution
```

The repository contains the compiler, runtime, standard library sources,
language specification, VS Code extension, examples, and release tooling.

## Why Ori exists

Ori optimizes for reading before writing.

Code should make important information visible at the point where the reader
needs it:

| Question | Ori makes it visible through |
|---|---|
| Where does this file belong? | `namespace` at the top of every file |
| What type does this value have? | explicit type annotations |
| Can this value be absent? | `optional<T>` |
| Can this operation fail? | `result<T, E>` |
| When is a resource released? | `using` |
| Where does behavior come from? | `trait` and `implement` |
| What went wrong? | structured diagnostic codes |

This design is especially important for readers who need lower cognitive load:
shorter inference chains, fewer hidden rules, and clearer error messages.

## Current status

| Area | Status |
|---|---|
| Version | `0.2.0`, frozen on the `0.2.x` line until there is a real breaking change |
| Stability | pre-1.0; source compatibility may still change |
| Compiler | Rust workspace with lexer, parser, HIR, type checker, codegen, diagnostics, LSP, driver, and runtime crates |
| Native backend | Cranelift object code plus the Ori native runtime |
| `ori run` | JIT by default when a runtime cdylib is available; AOT can be forced |
| `ori compile` | AOT native binary generation; default link route still depends on the configured linker strategy |
| C backend | debug/transpile route with partial feature parity |
| Standard library | Layer 1 runtime primitives plus Layer 2/3 `.orl` wrappers and algorithms |
| Tooling | CLI, formatter, diagnostics catalog, docs export, LSP, VS Code extension |
| Tests | workspace test suite and native release smoke are part of the project gate |

The project is intentionally conservative about versioning. `0.3.0` is reserved
for a real user-visible breaking change, not for every internal milestone.

## Quick start

Prerequisites for compiler development:

- Rust `1.95.0` from `rust-toolchain.toml`
- A platform linker or one of Ori's explicit linker strategies
- PowerShell on Windows for the release smoke scripts
- A C toolchain on Linux/macOS when using system discovery paths

From the repository root:

```bash
cargo check --workspace
cargo test --workspace
cargo run -p ori-driver -- check examples/hello_world.orl
cargo run -p ori-driver -- run examples/hello_world.orl
```

On Windows, validate a release-style package with:

```powershell
.\tools\smoke_native_release.ps1
```

Create a validated `.zip` package with:

```powershell
.\tools\package_native_release.ps1
```

On Linux or macOS:

```sh
sh tools/smoke_native_release.sh
```

Create a validated `.tar.gz` package with:

```sh
sh tools/package_native_release.sh
```

## A first program

```lua
namespace app.hello

import ori.io as io

func main()
    io.print("Hello, Ori!")

    const answer: int = 21 * 2
    io.print(f"The answer is {answer}")
end
```

Run it from this repository with:

```bash
cargo run -p ori-driver -- run examples/hello_world.orl
```

Ori uses `end`-delimited blocks, newline-separated declarations, explicit
imports, and explicit types for bindings and public contracts.

## CLI overview

The `ori` CLI is implemented by `compiler/crates/ori-driver`.

| Command | Purpose |
|---|---|
| `ori new <path>` | create a new app project skeleton |
| `ori check <file.orl>` | parse, resolve, and type-check a source file |
| `ori run <file.orl>` | compile and run through JIT or AOT, depending on runtime availability and env vars |
| `ori compile <file.orl>` | emit a native executable through the Cranelift backend |
| `ori test <file.orl> [--filter name]` | run functions marked with `@test`; `--filter` selects matching fully-qualified or short test names |
| `ori repl` | run a small interactive JIT-backed REPL |
| `ori fmt <file.orl>` | format source and print the formatted result |
| `ori doc file <file.orl>` | extract documentation comments as Markdown or HTML |
| `ori doc check <path>` | validate inline docs and `.oridoc` sidecar files |
| `ori doc export` | export stdlib symbols, diagnostics, and keywords as JSON |
| `ori doctor` | report stdlib, runtime, linker, target, and JIT health |
| `ori explain <code>` | explain a diagnostic code |
| `ori summary [path]` | print entry file, namespaces, imports, and diagnostics count |
| `ori build <path>` | build a file or project through the native backend |
| `ori emit c <file.orl>` | emit C through the partial debug backend |
| `ori lex <file.orl>` | print the token stream for compiler debugging |
| `ori parse <file.orl>` | print the AST for compiler debugging |
| `ori install <name> --path <dir>` | validate a local `ori.pkg.toml` package and copy it to the package cache |
| `ori publish <path>` | validate a package manifest; remote registry upload is not available yet |

Useful environment variables:

| Variable | Purpose |
|---|---|
| `ORI_STDLIB_ROOT` | override the `stdlib/` source root |
| `ORI_RUNTIME_LIB` | override the native runtime static library |
| `ORI_RUNTIME_CDYLIB` | override the runtime cdylib used by JIT |
| `ORI_USE_JIT=1` | force JIT for `ori run` |
| `ORI_USE_AOT=1` | force AOT for `ori run` |
| `ORI_USE_BUNDLED_RUST_LLD=1` | link through bundled `rust-lld` without the `rustc` driver |
| `ORI_USE_SYSTEM_LINKER=1` | link through the platform linker directly |
| `ORI_REQUIRE_PACKAGED_RUNTIME=1` | reject workspace runtime fallback during package validation |
| `ORI_PACKAGE_CACHE` | override the local package cache used by `ori install --path` |

The full environment matrix lives in [AGENTS.md](AGENTS.md).

## Project docs

Projects can use `ori.proj` as the entry point:

```ini
manifest = 1
name = "demo"
kind = "app"
entry = "src/main.orl"

[docs]
paths = ["docs/api"]
mode = "sidecar-first"
require_public = "off"
```

Long symbol documentation can live outside the `.orl` file:

```text
oridoc 1

namespace app.math

doc func add
    summary:
        Soma dois numeros.
    param left:
        Primeiro valor.
    param right:
        Segundo valor.
    returns:
        Soma dos valores.
end
```

See [Project and docs](docs/spec/17-project-and-docs.md) for the full
manifest and `.oridoc` contract. For a shorter workflow guide, use
[First project and local packages](docs/guides/first-project-and-packages.md).

## Language overview

Ori's core model is small:

- every file starts with `namespace`;
- imports create local aliases or explicit selective names with `only (...)`;
- top-level declarations are private unless marked `pub`;
- `struct` and `enum` define data;
- `trait` and `implement` define behavior;
- `optional<T>` models absence;
- `result<T, E>` models recoverable failure;
- `try` propagates `result` or `optional` values (`?` is the compact form);
- `using` makes cleanup explicit;
- diagnostics use stable codes such as `name.undefined` and
  `project.circular_import`.

Example with `result`:

```lua
namespace app.errors

import ori.io as io

func divide(a: int, b: int) -> result<int, string>
    if b == 0
        return error("division by zero")
    end

    return success(a / b)
end

func main() -> result<void, string>
    const value: int = try divide(84, 2)
    io.print(f"value: {value}")
    return success()
end
```

For the normative language contract, start with
[docs/spec/01-overview.md](docs/spec/01-overview.md).

## Compiler architecture

The compiler is split into focused crates:

| Crate | Role |
|---|---|
| `ori-lexer` | tokenization |
| `ori-ast` | AST node definitions |
| `ori-parser` | recursive descent parser |
| `ori-hir` | name resolution and lowered high-level IR |
| `ori-types` | type system, stdlib manifest, and checker contracts |
| `ori-codegen` | Cranelift native backend, JIT path, and C debug backend |
| `ori-runtime` | native runtime library and runtime ABI |
| `ori-diagnostics` | diagnostic codes and rendering support |
| `ori-lsp` | Language Server Protocol implementation |
| `ori-driver` | CLI, pipeline orchestration, integration tests |

The native runtime is the semantic reference for `ori compile`, `ori run`, and
`ori test`. The C backend is kept as a debug route and should not be treated as
the source of truth for async, ARC, collections, or runtime behavior.

## Standard library

The stdlib lives under the `ori.*` namespace.

Current shape:

| Layer | Location | Purpose |
|---|---|---|
| Layer 1 | `compiler/crates/ori-types/src/stdlib.rs` and `compiler/crates/ori-runtime/src/lib.rs` | manifest, ABI, hot runtime primitives |
| Layer 2 | `stdlib/**/*.orl` | safe wrappers over runtime primitives |
| Layer 3 | `stdlib/**/*.orl` | pure algorithms written in Ori |

Examples of available areas:

- `ori.io`, `ori.fs`, `ori.path`
- `ori.string`, `ori.bytes`, `ori.convert`
- `ori.list`, `ori.map`, `ori.set`
- `ori.math`, `ori.random`, `ori.time`
- `ori.json`, `ori.net`, `ori.process`
- `ori.task`, `ori.channel`, `ori.concurrent`
- `ori.test` and test helpers

See [stdlib/README.md](stdlib/README.md) for the current module inventory and
[docs/spec/12-stdlib.md](docs/spec/12-stdlib.md) for normative contracts.

## Editor tooling

Ori ships an LSP server and a VS Code extension under
[extensions/vscode-orl](extensions/vscode-orl/).

Implemented tooling includes:

- diagnostics from parser, resolver, and type checker;
- hover, go-to-definition, find references, and rename;
- semantic tokens, document symbols, workspace symbols, inlay hints;
- type-aware dot completion;
- stdlib-aware hover/completion/goto for Layer 1 and Layer 2 modules;
- formatting, code actions, code lens, signature help;
- incremental document sync;
- VS Code commands for check, run, test, format, doctor, and summary.

Build the extension locally with:

```bash
cd extensions/vscode-orl
npm install
npm run compile
```

Build the language server first:

```bash
cargo build -p ori-lsp -p ori-driver
```

## Repository layout

```text
ori-lang/
  compiler/crates/        Rust workspace for compiler, LSP, runtime, driver
  docs/spec/              normative language and implementation contracts
  docs/planning/          roadmap, backlog, and implementation plans
  stdlib/                 Ori standard library source modules
  runtime/                staged native runtime artifacts by target triple
  examples/               example Ori programs
  tests/                  end-to-end Ori fixtures and test documentation
  extensions/vscode-orl/  VS Code extension
  tools/                  staging, smoke, export, and validation scripts
  branding/               project logo assets
  _reversa_sdd/           historical reverse-engineering audit documents
```

## Development workflow

Common gates:

```bash
cargo check --workspace
cargo test --workspace
cargo test -p ori-driver --test diagnostic_catalog
cargo test -p ori-lsp
```

For stdlib changes:

```bash
cargo test -p ori-types --lib stdlib
cargo test -p ori-driver --test multifile_imports
```

For runtime or native backend changes, re-stage the runtime before running
compile/run integration tests:

```powershell
.\tools\stage_native_runtime.ps1
```

Unix:

```sh
./tools/stage_native_runtime.sh
```

Project rules:

- bug fixes need regression tests in `compiler/crates/ori-driver/tests/`;
- new behavior must update docs and `CHANGELOG.md`;
- new diagnostic codes must be registered in
  [docs/spec/13-error-catalog.md](docs/spec/13-error-catalog.md);
- stdlib runtime changes must keep the manifest, lowering, runtime ABI, tests,
  and docs in sync.

## Release layout

A release-style package is expected to keep this shape:

```text
ori.exe                         # or `ori` on Unix
ori-lsp.exe                     # or `ori-lsp` on Unix
stdlib/
  *.orl                         # packaged stdlib source modules
runtime/
  bin/
    rust-lld[.exe]              # optional bundled linker
  {target-triple}/
    ori_runtime.lib             # Windows MSVC static runtime
    libori_runtime.a            # Unix-style static runtime
    ori_runtime.dll             # Windows runtime cdylib for JIT
    libori_runtime.so           # Linux runtime cdylib for JIT
    libori_runtime.dylib        # macOS runtime cdylib for JIT
    runtime-link.json
examples/
README.md
```

The `native-route` workflow covers Windows MSVC, Windows GNU, Linux GNU,
macOS x86_64, and macOS aarch64. Runtime staging details live in
[runtime/README.md](runtime/README.md).

## Known limitations

Current pre-1.0 limitations:

- Ori is not self-hosting.
- `ori compile` is AOT and requires the platform linker (Visual Studio Build
  Tools on Windows, `build-essential` on Linux, Xcode Command Line Tools on
  macOS). `ori run` uses JIT by default and needs no linker.
- The compiler itself is written in Rust, so building Ori from source still
  requires Rust. End users who install via release package do not need Rust.
- C emission is partial and exists for debugging via `ori emit c`.
- `ori install --path` supports local packages and path dependencies, and the
  compiler resolves local package imports from `ori.proj` or `ori.pkg.toml`.
  Remote registry fetch and upload are still future work.
- `ori repl` is intentionally small: imports, simple `const`/`var` bindings,
  calls, literals, and simple expressions are supported first.
- Public contracts can still change before 1.0.

See [docs/planning/PENDENTES.md](docs/planning/PENDENTES.md) and
[docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md](docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md)
for the active backlog.

## Roadmap

Ori's long-term 1.0 criteria are deliberately strict:

1. remove the practical Rust dependency from end-user compilation paths —
   **mostly done**: `ori run` uses JIT (no linker), and `ori compile` defaults
   to the platform linker (no `rustc` or `rust-lld` required);
2. keep substantive stdlib layers in `.orl` where it makes sense —
   **in progress**: Layer 2/3 utilities and algorithms are already in `.orl`;
3. prove a self-hosting path or a credible bootstrap path —
   **deferred**: self-hosting is not a prerequisite for utility; a documented
   bootstrap path is an acceptable alternative;
4. document a stable ABI — **pending**;
5. gain real users beyond repository tests — **pending**;
6. avoid breaking changes for at least six months — **pending**.

Until then, the project stays honest about its pre-1.0 status.

## License

Ori is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
