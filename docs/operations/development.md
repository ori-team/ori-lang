# Development operations

All commands in this document assume the repository root unless stated otherwise.

## Prerequisites

- Rust toolchain pinned by `rust-toolchain.toml`;
- platform linker for native AOT workflows;
- C/C++ build tools required by the target platform and dependencies;
- PowerShell for Windows-specific packaging scripts;
- shell tools required by scripts under `tools/`.

The Cargo workspace is `compiler/Cargo.toml`.

## Initial validation

```bash
cargo --manifest-path compiler/Cargo.toml check --workspace
cargo --manifest-path compiler/Cargo.toml test --workspace
```

Focused mandatory gates:

```bash
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test diagnostic_catalog
cargo --manifest-path compiler/Cargo.toml test -p ori-lsp
```

Project fast gate:

```bash
sh tools/qa/daily_fast.sh
```

## Run the development compiler

```bash
cargo --manifest-path compiler/Cargo.toml run -p ori-driver -- check examples/hello/main.orl
cargo --manifest-path compiler/Cargo.toml run -p ori-driver -- run examples/hello/main.orl
cargo --manifest-path compiler/Cargo.toml run -p ori-driver -- compile examples/hello/main.orl
```

## Focused test routes

### Frontend and semantics

```bash
cargo --manifest-path compiler/Cargo.toml test -p ori-lexer -p ori-parser -p ori-types -p ori-hir
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test ori_spec
```

### Runtime and memory

```bash
cargo --manifest-path compiler/Cargo.toml test -p ori-runtime
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test memory_arc
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test concurrency_async
```

### Multifile, stdlib, and packages

```bash
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test multifile_imports
```

### JIT and native behavior

```bash
cargo --manifest-path compiler/Cargo.toml test -p ori-driver --test jit_run
```

## Runtime staging

Runtime source lives in `compiler/crates/ori-runtime`. Staged artifacts live under `runtime/<target-triple>/`.

Use the canonical scripts:

```bash
sh tools/stage_native_runtime.sh
```

Windows:

```powershell
.\tools\stage_native_runtime.ps1
```

After runtime or FFI changes, stage both:

- the static library used by AOT;
- the cdylib used by JIT.

A stale cdylib may make `ori run` behave differently from `ori compile`, including unsafe ABI mismatch.

## Environment overrides

Common development overrides include:

- `ORI_STDLIB_ROOT` — stdlib source root;
- `ORI_RUNTIME_LIB` — static runtime artifact;
- `ORI_RUNTIME_CDYLIB` — JIT runtime library;
- `ORI_USE_JIT=1` — force JIT route;
- `ORI_USE_AOT=1` — force AOT route;
- `ORI_USE_BUNDLED_RUST_LLD=1` — select bundled linker strategy;
- `ORI_USE_SYSTEM_LINKER=1` — select platform linker;
- `ORI_DISABLE_INCREMENTAL=1` — force native rebuild;
- `ORI_PACKAGE_CACHE` — package cache;
- `ORI_REGISTRY` — package registry source.

Use overrides deliberately and report them in bug or benchmark evidence.

## Working on the standard library

Review:

- `docs/architecture/stdlib.md`;
- `docs/spec/12-stdlib.md`;
- `docs/spec/15-stdlib-maintenance.md`;
- `stdlib/README.md`.

Run semantic, parity, multifile, native, and JIT tests appropriate to the operation.

## Working on diagnostics

- register emitted codes in Spec 13;
- add a negative test;
- validate primary span and action;
- run `diagnostic_catalog`;
- review CLI and LSP parity.

## Working on documentation

- begin with `docs/ATLAS.md`;
- update one canonical source rather than copying content;
- keep product version at 0.3.8 in active status documents;
- maintain language parallels where required;
- validate examples against current S3 syntax;
- update `docs/catalog.yaml` for new canonical documents;
- move completed plans to the archive.

## Troubleshooting route

### Native link failure

1. verify target triple and runtime metadata;
2. re-stage runtime;
3. confirm static library path;
4. confirm platform linker availability;
5. rerun the smallest native test;
6. inspect full linker command only after metadata and staging are confirmed.

### JIT symbol failure

1. verify `ORI_RUNTIME_CDYLIB` or packaged cdylib path;
2. re-stage cdylib;
3. compare runtime and compiler versions/ABI;
4. verify the symbol exists in the runtime manifest/export inventory;
5. run the focused JIT test.

### Workspace command fails at repository root

Use `--manifest-path compiler/Cargo.toml` or enter `compiler/` before invoking Cargo.

### Test contamination

Check environment variables, current directory, process-global caches, runtime staging, and temporary paths. Run the focused test in a fresh process before assuming an implementation defect.

## Completion evidence

Before opening a PR, record:

- branch and scope;
- commands executed;
- exact failures or passes;
- runtime staging performed;
- target and environment flags;
- docs and contracts updated;
- residual risk and skipped gates.