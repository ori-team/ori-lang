# Interoperability and the C ABI

> **Audience:** users embedding Ori or calling native libraries
> **Portuguese:** [interop.pt-BR.md](interop.pt-BR.md)
> **Normative source:** [16-runtime-ffi-safety.md](../spec/16-runtime-ffi-safety.md) and [19-abi.md](../spec/19-abi.md)

Ori has two different FFI directions. `extern` imports a native symbol for Ori
code to call. `@c_export` publishes a selected public Ori function for a C host
to call. Neither feature turns ordinary Ori functions into a stable C API.

## Exporting a library

```ori
module app.embed_add

@c_export
public add_scores(a: int, b: int) -> int
    return a + b
end

@c_export("mul_scores")
public mul(a: int, b: int) -> int
    return a * b
end
```

Build it with:

```bash
ori compile --lib examples/embed/add_scores.orl -o libadd_scores.so
```

The compiler writes the shared library and a sibling C header. Include the
generated header instead of reproducing declarations manually. The host must
call `ori_rt_init()` before using the library and `ori_rt_shutdown()` when it is
finished. The header also declares `ori_rt_version()` and
`ori_rt_abi_version()`; both return borrowed, NUL-terminated strings that the
host must not free. Check the ABI string before calling exports when the host
supports multiple Ori runtime revisions.

## Hosted compiler session foundation

> **0.3.8-dev safety block:** `OriValue` raw-pointer constructors remain
> explicitly `unsafe` borrowed escape hatches. Values returned by Ori carry a
> private ARC owner and are released on `Drop`; managed arguments are retained
> across callee cleanup, and `bytes` can be read with exact length through
> `as_bytes_with_len()`. The session remains experimental: do not manufacture
> pointers, retain borrowed raw values, or assume generation identity for a
> value created with a raw constructor.

Rust hosts can use the experimental `ori-embed` crate for a session boundary
with structured checking and a deliberately small persistent-JIT call surface:

- `OriConfig` selects target, execution profile, and declared/enabled features;
- `OriEngine` checks in-memory source and returns owned structured diagnostics;
- accepted modules receive stable `ModuleId` values and advancing generations;
- an invalid update leaves the last accepted generation intact;
- a valid update can compile public functions without `main`, resolve an opaque
  generation-bound handle, and call homogeneous `bool`, `int`, `float`,
  `slice`, `string`, or `bytes` signatures with at most four arguments.
- `OriHostRegistry` can bind a validated `extern host` scalar symbol once per
  session; the JIT caches its address instead of resolving it on every call.
- `OriHostRegistry::register_int_callback`, `register_float_callback`, and
  `register_bool_callback` can bind homogeneous scalar callbacks with opaque
  `user_data`; the JIT injects a stable callback ID and dispatches it without
  a per-call textual lookup. Each callback accepts up to four parameters of
  its registered scalar type and returns that same scalar type or `void` in
  this experimental Rust boundary.
- Callback removal is lifecycle-safe: a new call after `remove_callback`
  returns a structured cancellation trap, while removal during an active call
  returns `CallbackActive`. Reentry into the same `OriEngine` is supported for
  synchronous calls and recursion is bounded at 64 callback frames.
- `OriEngine::unload_module` releases the module's retained executable
  generations; handles then fail instead of calling freed code.
- scalar hosted traps (contracts, checks, integer division guards, and direct
  scalar collection/text bounds checks) return `OriExecutionError` through the
  Rust API; they do not unwind through the host or terminate the process.

This is not yet a general runtime execution API or a complete hot-reload
system. The generated C ABI-1 covers the `@c_export` surface described below,
but the hosted Rust session still does not expose aggregates or async
execution. Managed scalar pointers are owned only when returned by the JIT;
explicit raw constructors remain borrowed and unsafe. Non-scalar runtime abort
paths are not yet covered by the hosted trap mode. The callback slice is intentionally limited to trusted Rust
hosts and homogeneous scalar signatures; it does not yet define C header
callbacks, thread-affinity dispatch, object destruction, or callback migration
across reload. See the [host ABI plan](../planning/embedded-runtime-host-abi-v1.md)
and the [compiler service plan](../planning/interactive-compiler-service.md).

## Accepted boundary types

The generated native ABI-1 accepts:

- integer and floating-point scalars, `bool`, `void`, and NUL-terminated `string`;
- `bytes` through the generated `OriBytes { data, len }` view (exact length,
  including embedded NUL bytes);
- non-empty, non-generic scalar-field structs through pointer/out wrappers;
- managed structs through opaque ARC handles;
- direct `optional` and `result` bridges over those supported payloads.

Direct `list`, `map`, `set`, `tuple`, nested sum types, generic structs, and
empty structs are rejected. A collection can still be stored inside a managed
struct whose layout stays private behind an opaque handle.

Parameters carrying managed values are borrowed. Managed values returned to the
host transfer one owned reference; the host must release it with
`ori_arc_release`. Returned `string` values use the direct pointer form;
returned `bytes` values use `OriBytes *out` and transfer ownership through
`out->data`.

> **Foreign-string restriction:** the generated wrapper copies `string` inputs
> before passing them to Ori, including optional/result payloads. User code
> must still never manufacture or store a borrowed host pointer through a
> manual FFI escape; keep the pointer valid for the call only.

## `extern` and runtime FFI

`extern "C"` declarations use the platform C calling convention and exact
symbol names. Runtime-backed stdlib functions are declared in the compiler's
manifest and implemented by `ori-runtime`; user code should prefer the public
stdlib wrapper rather than calling runtime symbols directly.

The runtime safety contract forbids treating an arbitrary integer as a managed
pointer, retaining a borrowed value after its call, or inventing a collection
layout. See [16-runtime-ffi-safety.md](../spec/16-runtime-ffi-safety.md).

## Declarative native dependencies

In `ori.pkg.toml`, `[native.dependencies.<lib>]` (`pkg_config`, `static`, `framework`, `version`)
and `[native.linux]`, `[native.windows]`, `[native.macos]` (`libraries`, `frameworks`, `library_dirs`,
`link_flags`) declare system dependencies without manual `.a` linker script workarounds:

```toml
[native.dependencies.raylib]
pkg_config = "raylib"
version = ">= 5.0"

[native.linux]
libraries = ["GL", "X11", "m", "dl"]

[native.windows]
libraries = ["user32", "opengl32"]

[native.macos]
frameworks = ["OpenGL", "Cocoa"]
```

### Explicit alignment (`@align`)

`@repr("C")` structs can specify power-of-two alignment with `@align(N)` (1 to 64),
propagated into generated sibling C headers (`alignas(N)` / `__attribute__((aligned(N)))`)
for GPU uniform buffers and GDExtension boundaries:

```ori
@repr("C")
@align(16)
public struct UniformData
    matrix: int
    offset: int
end
```

## Current limits

- `@c_export` is native-backend functionality; the C/debug backend is not an ABI reference;
- callback registration is currently a Rust `ori-embed` experiment, not part
  of the generated C ABI-1 header;
- direct collection handles and nested sum types remain outside the contract;
- `ori-embed` still exposes only an experimental, unsafe managed-value path;
- symbol names must be portable, non-keyword C/C++ identifiers.

The complete smoke path lives in [`examples/embed/`](../../examples/embed/),
with a C host harness under `tests/native/`.
