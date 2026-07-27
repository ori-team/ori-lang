# Ori native runtime package

This directory is the release layout for the native runtime route.

`ori compile` first looks for the Rust `ori-runtime` static library here:

```text
runtime/{target-triple}/{runtime-artifact}
```

Examples:

```text
runtime/x86_64-pc-windows-msvc/ori_runtime.lib
runtime/x86_64-unknown-linux-gnu/libori_runtime.a
```

A complete release package should keep this shape:

```text
ori.exe                         # or `ori` on Unix
runtime/
  bin/
    rust-lld[.exe]              # bundled linker (optional, enables ORI_USE_BUNDLED_RUST_LLD=1)
  {target-triple}/
    {runtime-artifact}          # staticlib: ori_runtime.lib / libori_runtime.a
    {runtime-cdylib}            # cdylib:    ori_runtime.dll / libori_runtime.so / libori_runtime.dylib
    runtime-link.json
examples/
README.md
```

Each target directory should also contain `runtime-link.json`. That file records
the system libraries required by the Rust `staticlib` when a raw native linker is
used. It also records the Ori version and native ABI version used to stage the
runtime, so the driver can reject stale or incompatible runtime packages early.

The optional `runtime/bin/rust-lld[.exe]` (staged by `tools/stage_native_runtime.{ps1,sh}`
when `-SkipBundleLld`/`--skip-bundle-lld` is not set) lets users opt into the
`BundledRustLld` link strategy via `ORI_USE_BUNDLED_RUST_LLD=1`. When enabled,
`ori compile` invokes `rust-lld` directly and performs CRT discovery itself,
bypassing `rustc` entirely — so the end user does not need a Rust toolchain
installed just to link Ori programs. Supported on `x86_64-pc-windows-msvc`
(Rust removal Phase 1, Windows MSVC, via `vswhere.exe` + Windows SDK layout),
`x86_64-unknown-linux-gnu` (Rust removal Phase 1, Linux GNU, via
`cc -print-file-name`), and `x86_64-apple-darwin` / `aarch64-apple-darwin`
(Rust removal Phase 1, macOS, via `xcrun --show-sdk-path` + `-platform_version`).
Phase 1 is now complete for all three desktop OSes.

Users can also opt into the `SystemLinker` strategy via `ORI_USE_SYSTEM_LINKER=1`.
When enabled, `ori compile` invokes the platform system linker directly
(`link.exe` on Windows MSVC, `ld` on Linux GNU, `ld` via `xcrun` on macOS)
with the same CRT discovery as `BundledRustLld`, bypassing both `rust-lld` and
`rustc`. Override the linker path with `ORI_SYSTEM_LINKER`. Supported on
`x86_64-pc-windows-msvc` (via `vswhere.exe` + MSVC `link.exe` discovery),
`x86_64-unknown-linux-gnu` (via `cc -print-prog-name=ld`), and
`x86_64-apple-darwin` / `aarch64-apple-darwin` (via `xcrun --find ld`).
Phase 2 is now complete for all three desktop OSes.

`ori-runtime` now ships three crate types: `staticlib` (consumed by the AOT
link path), `rlib` (consumed by other Rust crates in the workspace), and
`cdylib` (consumed by the JIT path). The `cdylib` artifact —
`ori_runtime.dll` on Windows MSVC, `libori_runtime.so` on Linux GNU,
`libori_runtime.dylib` on macOS — is staged next to the staticlib and
recorded in `runtime-link.json` under the `runtime_cdylib` field.

Users can opt into the JIT execution path via `ORI_USE_JIT=1` (Rust removal
Phase 3). When enabled, `ori run` skips the AOT compile+link steps entirely:
the Cranelift `JITModule` lowers the HIR into executable memory in-process,
and the `ori_*` runtime symbols are resolved on demand from the staged
cdylib through `libloading`. No `.o` file is written, no linker is invoked,
no subprocess binary is spawned. `ori compile` and `ori test` remain AOT
(distribution requires a binary artifact; `ori test` requires process
isolation so `ori_test_assert` can `std::process::abort()` on failure).
Override the cdylib path with `ORI_RUNTIME_CDYLIB`. Phase 3 completes the
A→B→D hybrid for `ori run` — the driver can now execute Ori programs
without `rustc`, without a linker, and without writing any temporary
object file.

`ori-runtime` is the source of truth for native runtime semantics. The C backend
is a debug/transpile route and must not be used as the semantic reference for
`ori compile`, `ori test`, collections, ARC, or async/concurrency behavior.

Use this command from the repository root to stage the current host runtime:

```powershell
.\tools\stage_native_runtime.ps1
```

On Linux or macOS, use:

```sh
./tools/stage_native_runtime.sh
```

Use this command to verify a clean release-style package:

```powershell
.\tools\smoke_native_release.ps1
```

On Linux or macOS, use:

```sh
sh tools/smoke_native_release.sh
```

The smoke test copies `ori`, `runtime/`, and examples into a temporary folder
and runs with `ORI_REQUIRE_PACKAGED_RUNTIME=1`, so it does not silently fall back
to the workspace runtime.

The `.github/workflows/native-route.yml` workflow runs the native route against
the five CI triples below. Each triple has a staging command; the smoke step in
CI runs `tools/smoke_native_release.{ps1,sh}` with `ORI_REQUIRE_PACKAGED_RUNTIME=1`
to validate the staged runtime end-to-end.

| Triple | Runner | Stage command |
|---|---|---|
| `x86_64-pc-windows-msvc` | `windows-latest` | `.\tools\stage_native_runtime.ps1` |
| `x86_64-pc-windows-gnu` | `windows-latest` (GNU toolchain) | `.\tools\stage_native_runtime.ps1 -Target x86_64-pc-windows-gnu` |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `./tools/stage_native_runtime.sh` |
| `x86_64-apple-darwin` | `macos-15-intel` | `./tools/stage_native_runtime.sh --target x86_64-apple-darwin` |
| `aarch64-apple-darwin` | `macos-15` | `./tools/stage_native_runtime.sh --target aarch64-apple-darwin` |

Only `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu` runtime folders are
versioned in git as the canonical development baseline. The other three triples
are staged in CI and shipped with release packages, but not committed to the
repository — see `CONTRIBUTING.md` for the pre-built artifact policy.

Use this command to verify that the compiled static library exports every native
runtime symbol required by the stdlib manifest and backend:

```powershell
.\tools\check_native_runtime_exports.ps1
```

The static library itself is generated output and is ignored by git.
