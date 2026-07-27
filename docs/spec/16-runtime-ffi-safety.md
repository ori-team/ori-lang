# Runtime FFI safety contracts

> Audience: runtime maintainer, backend maintainer
> Status: current
> Surface: native runtime ABI

This file documents the shared safety rules for `unsafe extern "C"` runtime
functions. It is the domain-level contract used while `ori-runtime/src/lib.rs`
is still being split into smaller modules.

## General ABI rules

- All raw pointers passed to runtime functions must be either null when the
  function explicitly accepts null, or valid for the full operation.
- Pointers returned by allocation functions are owned by the caller according to
  the ARC contract in `10-memory.md`.
- Managed values stored inside another managed value must register an ARC edge.
- Removing or replacing managed children must unregister the old ARC edge.
- Runtime functions must not keep borrowed C pointers after the call returns
  unless the API explicitly copies the payload first.

## C export versus Rust visibility

Runtime entry points are a C ABI, not a public Rust library API. The
`#[no_mangle]` attribute gives each entry point its stable exported symbol in
the static and dynamic runtime libraries even when the Rust item itself is
private. Only Rust items intentionally consumed by another compiler crate,
such as `ORI_ABI_VERSION`, use Rust `pub` visibility.

This separation prevents Rust callers from bypassing the ABI contract while
preserving every `ori_*` symbol used by generated AOT and JIT code. The shared
pointer, length, lifetime, and ownership requirements in this chapter apply to
all exported entry points. Critical ARC primitives additionally keep local
`# Safety` rustdoc beside their implementation.

## String functions

Ori strings currently use a nul-terminated UTF-8 representation.

- `*const c_char` string inputs must point to valid nul-terminated UTF-8.
- Functions that create strings allocate a new managed string payload.
- Functions that return borrowed internal string pointers keep ownership in the
  source object. Callers must not free those pointers directly.
- String length, slice, and index APIs use character positions, not byte
  offsets, unless the function name states that it works on bytes.
- Inputs containing interior NUL are not valid Ori strings. Use `bytes` APIs for
  binary payloads.

## Bytes functions

Ori bytes are length-aware binary payloads.

- Bytes APIs must preserve `0x00` and must not use `CStr` to compute payload
  length.
- Inputs are valid when the data pointer is non-null for `len > 0`.
- A null data pointer is valid only when `len == 0`.
- UTF-8 decoding into `string` must reject interior NUL while strings remain
  nul-terminated.
- File APIs that read or write bytes must use the explicit bytes length.

## Collection functions

Collections own ARC edges to managed children.

- Insert paths must register managed children with the collection as owner.
- Remove, pop, clear, and replacement paths must unregister removed children.
- Map entries register both managed keys and managed values.
- Tree and graph node storage register managed node payloads in the runtime,
  not in backend-specific code.
- Iterator and snapshot APIs that expose managed values must retain or preserve
  ownership according to the returned value contract.

## Heap comparator functions

Custom heap comparison calls may retain managed values temporarily before
calling user code.

- Each temporary retain must have a matching release after the comparison.
- Comparator failures must not skip release cleanup.
- Repeated comparisons must not increase the refcount of heap items.

## Cycle collector

The runtime ships a trial-deletion cycle collector accessible via
`ori_arc_collect_cycles()`.

- The collector only reclaims objects whose trial-deletion refcount reaches
  zero (i.e. objects reachable only from themselves). Objects with external
  references are never collected.
- The collector calls each reclaimed object's destructor (if any) before
  freeing the header, which cascades releases to owned edges.
- Collected objects are removed from the allocation registry before any
  destructor runs, so a destructor that releases a sibling cycle member is a
  no-op (the sibling is already unregistered).
- The collector is not currently invoked on a periodic schedule. It runs at
  specific safe points (see `docs/spec/10-memory.md` — Cooperative collection
  points) and via explicit `ori.test.collect_cycles()` calls.
- FFI code that manually registers edges must ensure every registered edge is
  eventually unregistered or that the owner is collected; otherwise the
  collector cannot prove the cycle is unreachable.

## Leak check FFI

- `ori_test_live_allocations()` returns the live allocation count without
  running the collector. Safe to call from any thread.
- `ori_test_collect_cycles()` runs the collector and returns the number of
  objects reclaimed.
- `ori_test_assert_no_leaks(label)` runs the collector, then returns the live
  count. When `ORI_TEST_LEAK_CHECK=1` is set in the environment and the count
  is non-zero, it prints a diagnostic and aborts. The `label` is a
  null-terminated C string used in the diagnostic.

## Source-level rustdoc policy

Critical ARC and memory functions keep local `# Safety` rustdoc near the
function. Broad FFI families use this chapter as their shared contract. When a
runtime entry point becomes a public Rust API, it must also gain function-level
`# Safety` rustdoc before its visibility is widened.

## Native link strategies

The native backend (`ori-codegen::native_backend`) supports four link strategies,
selected by `NativeLinker::discover()` in priority order:

| Priority | Env var | Strategy | Linker | CRT discovery |
|----------|---------|----------|--------|---------------|
| 1 | `ORI_NATIVE_LINKER` | `RawNativeCommand` | User-specified path | None (escape hatch) |
| 2 | `ORI_USE_BUNDLED_RUST_LLD=1` | `BundledRustLld` | `rust-lld` (bundled or sysroot) | Yes (Phase 1) |
| 3 | `ORI_USE_SYSTEM_LINKER=1` | `SystemLinker` | `link.exe` / `ld` / `ld64` | Yes (Phase 2) |
| 4 | (default) | `RustcDriver` | `rustc` as link driver | Delegated to rustc |

Phase 1 (`BundledRustLld`) and Phase 2 (`SystemLinker`) both perform
compiler-side CRT discovery without requiring `vcvarsall.bat` (Windows) or a Rust
toolchain. Phase 2 eliminates the `rust-lld` dependency when the platform system
linker is available. Override paths: `ORI_RUST_LLD` (Phase 1), `ORI_SYSTEM_LINKER`
(Phase 2).

## JIT execution path (Rust removal Phase 3)

In addition to the four AOT link strategies above, `ori run` supports a JIT
execution path that bypasses the link step entirely. Opt in with
`ORI_USE_JIT=1`.

| Aspect | AOT path (`ori compile` / `ori test` / default `ori run`) | JIT path (`ori run` with `ORI_USE_JIT=1`) |
|--------|------------------------------------------------------------|---------------------------------------------|
| Cranelift module | `ObjectModule` → `.o`/`.obj` file | `JITModule` → in-memory executable code |
| Runtime symbol resolution | Static link against `staticlib` (`ori_runtime.lib` / `libori_runtime.a`) | Dynamic lookup against `cdylib` (`ori_runtime.dll` / `libori_runtime.so` / `libori_runtime.dylib`) via `libloading` |
| Linker | `rust-lld`, `link.exe`, `ld`, or `rustc` (per strategy above) | None |
| Process model | Subprocess binary (`.exe` / ELF / Mach-O) | In-process function pointer call |
| Output artifact | Persistent binary (compile) or temp binary (run/test) | None |

The JIT path requires the runtime cdylib to be staged alongside the staticlib
(see `runtime/README.md`). `find_native_runtime_cdylib()` in the driver resolves
the cdylib path with the same search order as the staticlib (`ORI_RUNTIME_CDYLIB`
override → packaged → cargo fallback).

`ori compile` and `ori test` remain AOT-only:
- `ori compile` produces a distributable binary artifact; JIT'd code cannot be
  shipped.
- `ori test` requires process isolation so `ori_test_assert` can call
  `std::process::abort()` on failure without taking down the driver process.

`NativeBackend<M: Module>` is generic over the Cranelift module type so the
same HIR-lowering code (`prepare()`) feeds both the AOT (`ObjectModule`) and
JIT (`JITModule`) paths. The AOT path adds `compile()` (calls `prepare()` then
`module.finish().emit()`); the JIT path calls `prepare()`, then
`module.finalize_definitions()` and `module.get_finalized_function(main_id)` to
retrieve the entry pointer.

If an Ori program calls `os.exit(code)` under JIT, the runtime invokes
`std::process::exit(code)` and the driver process terminates with that code —
matching AOT `ori run` semantics. JIT'd code that panics or segfaults also
terminates the driver (acceptable for an opt-in mode).
