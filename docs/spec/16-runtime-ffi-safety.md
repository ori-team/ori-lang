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

## `handle[T]` boundary

`handle[T]` is the source-level spelling for a borrowed, opaque host pointer.
It is intentionally weaker than an ARC-managed Ori value:

- the runtime does not retain or release the pointee;
- the handle is not `Transferable` and cannot cross a spawned-task boundary;
- a null pointer (`0`) is representable, and `ori.handle.is_null` provides a
  non-dereferencing probe for that sentinel; there is still no dedicated
  nullable constructor or safe dereference operation;
- `==` and `!=` compare pointer identity only; they never dereference or retain
  the pointee. This is not semantic equality, and handles are not hashable by
  this rule;
- generated FFI wrappers may borrow the pointer only for the duration of the
  call. They must not store it, return it after the host-owned object expires,
  or expose it to another thread.
- `@c_export` wrappers validate opaque managed handles against the runtime ARC
  registry before retaining or dereferencing them. The compiler emits a
  concrete payload size and source-type tag for every managed aggregate export;
  null, foreign, wrong-size, and wrong-type pointers take the deterministic
  bounds-failure path instead of reaching user code. There is no generated
  provenance-only fallback for generic or legacy untyped handles: such a
  boundary is rejected during code generation.

The compiler currently checks the shape of `handle[T]`, preserves its pointer
representation, rejects it at task/channel transfer boundaries, and rejects
`@c_export` aggregates that would retain it past a host call. The
`ori.handle.is_null` helper is safe because it only compares the pointer with
null; it does not validate or retain the pointee. The compiler still does not
prove pointee lifetime, nullable construction, or host-thread affinity.
Callers therefore own those invariants. A future contract (`LANG-HANDLE-1`)
must add explicit nullable handling, safe access helpers, and compile-time
rejection of escaping borrows before this boundary can be considered complete.

## String functions

Ori strings currently use a nul-terminated UTF-8 representation.

- `*const c_char` string inputs must point to valid nul-terminated UTF-8.
- If a legacy pointer-returning entry point receives invalid UTF-8, it records
  host error `1003` (`ORI_HOST_ERROR_INVALID_UTF8`) in the current thread's
  hosted-error slot and returns its documented compatibility value. Hosts must
  inspect and clear that slot; new typed boundaries must return an explicit
  error instead of using the compatibility value.
- If a runtime worker cannot be created, it records host error `1004`
  (`ORI_HOST_ERROR_THREAD_SPAWN`) with the worker name and operating-system
  cause. The associated future still reaches `Failed`; hosts should inspect the
  error slot before starting another operation.
- A worker or callback panic is caught only at a declared `C-unwind` boundary.
  It produces a terminal failed future or structured callback trap and a
  readable runtime diagnostic; it must never unwind through an `extern "C"`
  dispatcher.
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
- Legacy pointer-only byte entry points reject unregistered foreign pointers
  with host error `1002` (`ORI_HOST_ERROR_INVALID_ARGUMENT`) instead of
  probing memory for a terminator. Hosts must use the explicit `(data, len)`
  `OriBytes` view for foreign buffers.
- Inputs are valid when the data pointer is non-null for `len > 0`.
- A null data pointer is valid only when `len == 0`.
- UTF-8 decoding into `string` must reject interior NUL while strings remain
  nul-terminated.
- File APIs that read or write bytes must use the explicit bytes length.
- Generated `@c_export` headers represent bytes as `OriBytes { data, len }`.
  The wrapper copies the view before calling Ori; hosts must keep `data` valid
  for the call and must release returned `out->data` with `ori_arc_release()`.

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

## Hosted runtime lifecycle

- Runtime-created threads attach a per-thread stack-guard altstack and detach
  it on exit. Foreign threads must bracket calls with
  `ori_rt_thread_attach()`/`ori_rt_thread_detach()`.
- `__ori_module_init()` constructs dynamic globals once per generation;
  `__ori_module_shutdown()` releases their managed slots before code unload.
- `ori_rt_shutdown_ex(timeout_ms)` cancels persistent queues, joins persistent
  workers, drains executor closures, waits for detached jobs, and rejects
  shutdown while another foreign thread remains attached. Only return value
  zero permits `dlclose`/`FreeLibrary`.
- Error `1006` means workers remain reachable and the cdylib must stay loaded.
- Linux signal handlers cache page size before installation, use only
  async-signal-safe operations in the handler, preserve the previous action,
  and restore it at successful shutdown.

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

Before a staged cdylib is loaded, codegen validates its filename, target,
package version, ABI revision, and SHA-256 against `runtime-link.json`. It then
queries `ori_rt_version`, `ori_rt_abi_version`, and `ori_rt_target` from the
loaded library before registering runtime symbols. Missing or mismatched
identity fails with `native.abi_mismatch`; staged metadata without a digest is
not accepted. Staging with `--skip-build` refuses an artifact older than the
runtime source tree.

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
