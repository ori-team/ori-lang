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
finished.

## Accepted boundary types

ABI-1 accepts:

- integer and floating-point scalars, `bool`, `void`, and `string`;
- non-empty, non-generic scalar-field structs through pointer/out wrappers;
- managed structs through opaque ARC handles;
- direct `optional` and `result` bridges over those supported payloads.

Direct `list`, `map`, `set`, `tuple`, nested sum types, generic structs, and
empty structs are rejected. A collection can still be stored inside a managed
struct whose layout stays private behind an opaque handle.

Parameters carrying managed values are borrowed. Managed values returned to the
host transfer one owned reference; the host must release it with
`ori_arc_release`. A returned `string` follows the same ownership rule.

## `extern` and runtime FFI

`extern "C"` declarations use the platform C calling convention and exact
symbol names. Runtime-backed stdlib functions are declared in the compiler's
manifest and implemented by `ori-runtime`; user code should prefer the public
stdlib wrapper rather than calling runtime symbols directly.

The runtime safety contract forbids treating an arbitrary integer as a managed
pointer, retaining a borrowed value after its call, or inventing a collection
layout. See [16-runtime-ffi-safety.md](../spec/16-runtime-ffi-safety.md).

## Current limits

- `@c_export` is native-backend functionality; the C/debug backend is not an ABI reference;
- callbacks from a host into Ori exports are not part of ABI-1;
- direct collection handles and nested optional/result bridges remain outside the contract;
- symbol names must be portable, non-keyword C/C++ identifiers.

The complete smoke path lives in [`examples/embed/`](../../examples/embed/),
with a C host harness under `tests/native/`.
