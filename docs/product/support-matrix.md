# Support matrix

> Current project version: **0.3.8**  
> This matrix describes repository-supported and validated routes. It is not a guarantee that every feature works identically on every host.

## Support levels

| Level | Meaning |
|---|---|
| **Reference** | Defines language semantics and receives complete compatibility attention |
| **CI validated** | Built and exercised by repository automation for the stated route |
| **Supported** | Intended for normal use with documented prerequisites |
| **Best effort** | Implemented, but validation or platform coverage is incomplete |
| **Partial** | Deliberately supports a documented subset |
| **Deferred** | Not in the current execution queue |

## Execution backends

| Route | Level | Contract |
|---|---|---|
| Cranelift native AOT | **Reference** | Semantic reference; produces native objects and linked executables/libraries |
| Cranelift in-process JIT | **Supported / CI validated where packaged cdylib exists** | Shared language semantics with AOT; no linker required for ordinary `ori run` |
| Bytecode VM | **Deferred / not a product** | Ori is AOT-first and does not define a bytecode compatibility contract |

## Target and package matrix

| Target | Native build | Runtime staging | JIT cdylib | Package/smoke intent | Notes |
|---|---|---|---|---|---|
| `x86_64-unknown-linux-gnu` | CI validated | versioned development baseline | supported | `.tar.gz` and `.deb` routes | Primary Linux development target |
| `x86_64-pc-windows-msvc` | CI validated | versioned development baseline | supported | `.zip`/installer route | Requires documented Windows toolchain for AOT |
| `x86_64-pc-windows-gnu` | CI generated | CI staging | intended | release matrix route | Not a versioned local baseline |
| `x86_64-apple-darwin` | CI generated | CI staging | intended | release matrix route | Requires macOS linker/tooling for AOT |
| `aarch64-apple-darwin` | CI generated | CI staging | intended | release matrix route | Host architecture selection must use staged aarch64 runtime |
| Other triples | Best effort or unsupported | not guaranteed | not guaranteed | none | Requires explicit target work and evidence |

A target is not promoted to supported solely because Rust/Cranelift can emit an object. Runtime, linking, execution, package, and smoke evidence are required.

## Tooling

| Tool | Level | Notes |
|---|---|---|
| CLI check/run/compile/test | Supported | Core public workflow |
| Formatter | Supported | Must remain parse-compatible and idempotent |
| Documentation extraction/check/export | Supported | Source comments, sidecars, stdlib/diagnostic export |
| LSP | Supported | Reuses compiler semantics for diagnostics and navigation |
| VS Code/Cursor extension | Local supported workflow | Installed from repository-built extension; marketplace publication is not current priority |
| Zed extension | Local/dev workflow | Language-server integration; debugger integration depends on editor API support |
| Cooperative debugger/DAP | Supported with documented limits | Portable debug map and runtime instrumentation; native rich-variable formats remain target/toolchain-dependent |
| REPL | Best effort / pre-1.0 | JIT-backed with documented limitations |
| Self-update | Supported only for documented release layouts | Must verify artifact metadata/checksum and refuse incompatible install modes |

## Language feature support

The canonical feature-by-backend matrix is [`../spec/14-backend-support.md`](../spec/14-backend-support.md).

General rule:

- native AOT defines reference behavior;
- JIT matches the shared native surface;
- unsupported native shapes must reject explicitly;
- LSP/formatter support is required for public syntax where applicable.

## Installation prerequisites

### JIT execution

A packaged compatible runtime cdylib is required. The Rust toolchain and platform linker are not required for the ordinary JIT route.

### AOT compilation

A usable linker/toolchain is required unless an appropriate bundled linker route is present and selected.

Examples:

- Linux: system C/build toolchain;
- Windows: Visual Studio Build Tools or the documented GNU route;
- macOS: Xcode Command Line Tools.

Exact installation instructions belong in `docs/install.md` and its maintained translations.

## Evidence requirements

Every matrix promotion should link or be backed by:

- CI workflow/job;
- target-specific build;
- staged static runtime;
- staged cdylib when claiming JIT;
- isolated smoke package;
- representative executable result;
- version and ABI metadata validation;
- documented prerequisites and limitations.

## Update policy

Update this matrix when:

- a target enters or leaves CI;
- a package format becomes public;
- AOT/JIT/tooling support changes;
- editor installation/distribution changes;
- a route becomes reference, partial, experimental, or deferred;
- prerequisites materially change.

Release notes describe what changed; this document describes current support.