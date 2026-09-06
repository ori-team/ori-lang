# Ori Language Specification — Chapter 19: Native ABI

> Status: **normative for the native backend** · **ABI-1 in force** (FREEZE-1 closed)
> Audience: compiler implementers, runtime maintainers, FFI authors  
> Surface: **S3** (`0.3.0`) + inference **`0.3.1`** · workspace **`0.3.8-dev`
> Revision tag: **`ori-native-abi-1`** (`ORI_ABI_VERSION` in `ori-runtime`)  
> Source of truth: `compiler/crates/ori-runtime/src/lib.rs` + `ori-codegen` native backend  
> Process: [freeze-and-abi-gates.md](../planning/freeze-and-abi-gates.md)  
> Related: [10-memory.md](10-memory.md), [16-runtime-ffi-safety.md](16-runtime-ffi-safety.md), [18-stability-and-compatibility.md](18-stability-and-compatibility.md)

---

## 1. Purpose and scope

This chapter is the **M3 ABI contract**: what memory layouts, symbol names, and
calling conventions the native pipeline actually uses today, so that:

1. Cranelift object code and `libori_runtime.a` / `libori_runtime.so` stay linked.
2. Future changes to layouts are deliberate (bump `ORI_ABI_VERSION` + CHANGELOG).
3. External C code that calls runtime symbols has a documented contract.

**Out of scope here:**

- Self-hosting / interpreter ABI (none).
- Guaranteeing that every user-defined Ori function is a stable C export for
  third parties (only entry `main` and `extern "C"` are unmangled exports).

**In scope:**

- Primitive and composite value layouts emitted by the native backend.
- ARC heap header and managed type representations.
- Runtime collection structs (`OriList`, `OriMap`, `OriSet`, …).
- Symbol mangling for Ori functions and globals.
- Link-time ABI versioning via `runtime-link.json`.
- Calling convention for `extern "C"` and Cranelift-emitted functions.

---

## 2. ABI version

| Item | Value |
|------|--------|
| Constant | `ORI_ABI_VERSION` in `ori-runtime` |
| Current string | **`ori-native-abi-1`** |
| Consumer | `ori-driver` embeds the same string in staged `runtime-link.json` |
| Check | Driver rejects a staged runtime whose `abi_version` ≠ driver constant |

The loaded runtime also exposes two process-lifetime, NUL-terminated C strings:

| Symbol | Contract |
|--------|----------|
| `ori_rt_version() -> const char *` | Cargo package version of the runtime (for diagnostics and host telemetry) |
| `ori_rt_abi_version() -> const char *` | The exact ABI revision string, currently `ori-native-abi-1` |

Hosts must treat both pointers as borrowed immutable strings. They must not free
or mutate them. A host that loads a shared library should compare
`ori_rt_abi_version()` with the ABI revision it was built to consume before
calling exported functions.

The runtime also exposes a thread-local hosted-error slot. Generated hosted JIT
code writes controlled failures with `ori_host_report_error`; hosts may clear
and inspect the current-thread result with:

| Symbol | Contract |
|--------|----------|
| `ori_host_clear_error()` | Clear the current-thread error |
| `ori_host_error_code() -> int32_t` | Return zero when no hosted error is pending |
| `ori_host_error_message() -> const char *` | Borrowed NUL-terminated message until the next error operation |

The runtime reserves host error code `1002` (`ORI_HOST_ERROR_INVALID_ARGUMENT`)
for a legacy pointer-only byte boundary that receives an unregistered foreign
pointer, and `1003` (`ORI_HOST_ERROR_INVALID_UTF8`) for
an invalid UTF-8 NUL-terminated string supplied at a legacy C-string boundary,
and `1004` (`ORI_HOST_ERROR_THREAD_SPAWN`) for a runtime worker that could not
be created. Code `1004` includes the worker name and operating-system cause in
the thread-local message slot; the affected future is terminal `Failed`. The
current compatibility entry points return their existing empty/sentinel value
after recording these errors; hosts must check the slot before treating the
call as successful.

This slot is an execution aid for the experimental Rust hosted API, not a
general recovery mechanism for arbitrary native faults. Standalone AOT/JIT
keeps the existing abort policy.

When any **documented layout or stable `ori_*` symbol signature** in this chapter
changes in a way that breaks binary compatibility with previously staged
runtimes, maintainers must:

1. Bump the revision string (e.g. `ori-native-abi-2`).
2. Note the change in `CHANGELOG.md`.
3. Re-stage runtime staticlib + cdylib for all packaged triples.

Additive runtime symbols that old object code never called do not require a
version bump, but should still be documented here or in the stdlib manifest.

---

## 3. Target and calling convention

| Aspect | Contract |
|--------|----------|
| Word size | 64-bit only on supported desktop triples (`x86_64-*`, `aarch64-apple-darwin`) |
| Pointer size | 8 bytes |
| Default CC | Platform C ABI (System V AMD64 / Microsoft x64 / Apple AArch64) |
| Cranelift functions | Declared with the system default C calling convention |
| `extern "C"` imports | Same platform C ABI; symbol name is exact (no Ori mangling) |
| Runtime exports | `#[no_mangle] unsafe extern "C"` — C symbol = Rust `fn` name (`ori_list_new`, …); Rust visibility is private unless another compiler crate intentionally consumes the item |

Managed values are almost always passed as **payload pointers** (`*mut u8` /
`i64` bit-pattern of a pointer). Primitive `int`/`float`/`bool` use native
integer and IEEE floats as below.

---

## 4. Primitive layouts

| Ori type | Native representation | Size | Notes |
|----------|----------------------|------|--------|
| `bool` | `i8` | 1 | `0` = false, non-zero treated as true at boundaries; codegen uses 0/1 |
| `int` | `i64` | 8 | Signed 64-bit |
| `float` | `f64` | 8 | IEEE-754 binary64 |
| `void` | empty / ignored | 0 | Not a value; result `ok()` may store null payload |
| Function pointers | pointer | 8 | Platform function pointer |
| Raw pointers (FFI) | pointer | 8 | As declared in `extern` bindings |

Alignment for a field of size *N* bytes (backend helper `field_size_align`):

```text
align = min(N, 8).max(1)
```

So `bool` aligns to 1, `int`/`float`/pointers align to 8 on current targets.

---

## 5. Composite layouts (codegen)

All offsets and sizes below are computed by the native backend
(`compute_struct_layout`, `compute_enum_layout`, `optional_layout`,
`result_layout`, `tuple_layout`, `lazy_layout`).

### 5.1 Structs

- Field order = declaration order (no reordering).
- Default and `repr(C)` path: **natural alignment** (pad each field to its
  align; pad total size to struct max align).
- Packed path (`repr_c = false` in HIR): no inter-field padding (legacy /
  special cases).
- Size is at least 1 if the struct is non-empty layout path requires storage.

C mental model for default structs:

```c
struct User {   /* fields in source order, C-like padding */
    /* ... */
};
```

### 5.2 Tuples

Anonymous struct of elements left-to-right with natural alignment. Same rules
as structs for offsets and total size.

### 5.2b Inline structs inside `array`

Since 0.3.8 (GFX-INLINE-1), a struct whose fields are all inline (`Inline(T)`:
scalars, inline arrays, inline structs) can be an `array[T, size: N]` element
or an inline field of another struct. Such structs are laid out **in place**:

- element stride = the struct's full size (natural alignment), so
  `array[Vec3, size: 8]` is `8 × sizeof(Vec3)` contiguous bytes;
- an inline struct field occupies its bytes at its natural-aligned offset
  (never a pointer);
- managed structs (any ARC field) and recursive structs stay pointer-backed /
  rejected, so no inline layout ever holds an ARC edge.

`ori.mem.size_of` reports the same numbers the codegen layout uses.

### 5.3 Enums (tagged unions)

User-defined `enum` with or without payloads:

| Part | Representation |
|------|----------------|
| Discriminant tag | **`i32`** at offset 0 (size 4, align 4) |
| Variant index | Declaration order, starting at **0** |
| Payload | Union of per-variant field structs, starting at `payload_offset` |
| `payload_offset` | `align_up(4, max_payload_align)` — natural align, **not** packed |
| Total size | `align_up(payload_offset + max_payload_size, overall_align)`, at least 4 |

Payload field layouts use **natural alignment** so pointer-bearing variants
(e.g. `ori.json.Value`) keep payloads at pointer-aligned offsets (commonly
offset 8 when max payload align is 8).

C mental model:

```c
struct EnumValue {
    int32_t tag;
    /* padding to payload_offset */
    union {
        /* per-variant payloads */
    } payload;
};
```

This is **not** Rust `#[repr(C, u8)]` (tag is 32-bit, not 8-bit). Older
aspirational text that claimed a 1-byte tag was incorrect for the native backend.

### 5.4 `optional[T]` (codegen layout)

```text
{ has_value: i8, [padding], value: T }
```

- `value_offset = align_up(1, align_of(T))`
- `total = align_up(value_offset + size_of(T), align_of(T))`, minimum 2

When `T` is a managed pointer type, runtime helpers often allocate a
**pointer-sized** optional box (see §7) that is ABI-compatible with “flag +
pointer payload at word offset”.

### 5.5 `result[T, E]` (codegen layout)

```text
{ is_ok: i8, [padding], union { ok: T, err: E } }
```

- `payload_offset = align_up(1, max(align_of(T), align_of(E)))`
- `payload_size = max(size_of(T), size_of(E))`
- `total = align_up(payload_offset + payload_size, max align)`, minimum 2

`is_ok != 0` means Ok arm; `0` means Err arm.

Runtime constructors for pointer-shaped results often use a simplified
**word-aligned box** (see §7.3).

### 5.6 `lazy[T]`

```text
{ thunk: ptr, forced: i8, [padding], value: T }
```

- Value starts at `align_up(ptr_size + 1, align_of(T))`.

### 5.7 Closures

Environment is a packed capture record (offsets via `closure_env_layout`).
A closure value at runtime is typically two words: function pointer + env
pointer (see task spawn path in runtime).

---

## 6. ARC heap model

### 6.1 Header (`OriHeapHeader`)

Every object allocated with `ori_alloc` is:

```text
[ OriHeapHeader | payload ... ]
                 ^── pointer returned to callers (payload start)
```

```c
/* Rust: #[repr(C)] OriHeapHeader */
struct OriHeapHeader {
    int64_t refcount;   /* AtomicI64 in Rust */
    void (*destructor)(uint8_t *payload);  /* optional; may be null */
};
```

| Field | Role |
|-------|------|
| `refcount` | Starts at 1; `ori_arc_retain` / `ori_arc_release` |
| `destructor` | Called with the **payload** pointer when refcount hits 0, before free |

Callers never pass the header pointer to retain/release — only the payload.

**Note:** A historical comment in the runtime mentioned `[u32 ref][u32 type_tag]`.
That is **obsolete**. The live header is refcount + optional destructor function
pointer. The hook is used by runtime-internal cleanup and compiler callbacks
for `core.Destructor`; cascading release of stored managed children still goes
through registered ARC edges (single cascade owner — see
`docs/planning/adr-arc-single-cascade-owner.md` and Spec 10).
Layout guard test: `ori_heap_header_layout_is_stable` (`ori-runtime`).

Concrete user `struct`/`enum` payloads allocated by native code use the
compiler-private `ori_alloc_typed(size, destructor, type_tag)` entry point.
`type_tag` lives in the ARC registry, not in the public header, and is checked
by `ori_handle_validate_size_type` at `@c_export` ingress. Runtime-owned and
legacy allocations use tag `0` and are not valid substitutes for a typed
export handle.

### 6.2 Core ARC API (stable symbols)

| Symbol | Contract |
|--------|----------|
| `ori_alloc(size, destructor)` | Allocate header+payload; register; return payload; refcount = 1 |
| `ori_alloc_typed(size, destructor, type_tag)` | Compiler-private allocation with a non-zero source-type tag in the ARC registry |
| `ori_arc_retain(ptr)` | No-op if null or not registered |
| `ori_arc_release(ptr)` | No-op if null or not registered; free at 0 after dtor |
| `ori_arc_register_edge(owner, child)` | Strong edge owner→child for cycle GC; retains child |
| `ori_arc_unregister_edge(owner, child)` | Remove edge; release child edge retain |
| `ori_arc_collect_cycles()` | Trial-deletion collector; returns reclaimed count |
| `ori_handle_validate_size_type(ptr, size, type_tag)` | Compiler-private ingress guard for concrete exported aggregate handles; aborts on invalid provenance, size, or tag |

The standard-library helper `ori.handle.null()` lowers to
`ori_handle_null()`, which returns the null pointer sentinel for a borrowed
`handle[T]`. It carries no ARC ownership and is safe only for comparisons and
`ori.handle.is_null`; it does not make a host resource live.

Detailed safety rules: [16-runtime-ffi-safety.md](16-runtime-ffi-safety.md).  
Language-level ARC rules: [10-memory.md](10-memory.md).

### 6.3 Registration

Only payloads returned by `ori_alloc`/`ori_alloc_typed` (or constructors that
call them) are
registered. Static/literal C strings and some runtime `malloc` boxes are
**not** ARC-managed; retain/release ignore them.

---

## 7. Managed language types

In Ori values, managed types are **references** (payload pointers). Assigning
copies the reference and the backend inserts retain/release.

### 7.1 `string`

- Representation: `*mut u8` → **NUL-terminated UTF-8** payload from `ori_alloc`
  (length = `registered_size - 1`, or `strlen` fallback for non-registered).
- Interior NUL is not a valid Ori string; use `bytes` for binary data.
- Index/slice APIs are **character**-oriented unless the symbol name says bytes.

### 7.2 `bytes`

- Length-aware binary payload (not C string semantics).
- Null data pointer only valid when length is 0.
- May include `0x00` bytes; must not use `CStr` to measure length.

### 7.3 Runtime boxes for `optional` / `result` (FFI helpers)

Several runtime helpers allocate simplified boxes (often `2 * sizeof(void*)`):

```text
offset 0:          flag byte (has_value or is_ok), rest of first word zeroed
offset ptr_size:   payload word (pointer or i64/f64 bit pattern)
```

Examples:

- `new_optional_ptr` → `ori_alloc` (ARC-managed optional of pointer).
- `new_result` / `ori_new_result` → plain `malloc` box (not always ARC-registered).
- `new_result_raw` / `new_result_i64_ok` → same 2-word shape with unaligned write of i64/f64.

**Important for FFI authors:** prefer calling the documented `ori_*` entry points
rather than hand-building these boxes. When codegen and runtime disagree on
whether a box is ARC-registered, the backend’s retain/release path is the
reference for Ori-compiled code.

### 7.4 `list[T]` — `OriList`

```c
typedef struct OriList {
    int64_t *data;   /* elements as i64 bit patterns / pointers */
    int64_t  len;
    int64_t  cap;
    int64_t  version; /* bump on structural mutation (iterators) */
} OriList;
```

- Object itself is ARC-allocated with `ori_list_dtor` (frees `data` buffer).
- Elements that are managed register edges list→element on insert.
- `ori_list_new`, `ori_list_with_capacity`, `ori_list_reserve`,
  `ori_list_capacity`, `ori_list_push`, `ori_list_get`, `ori_list_len`, …

### 7.5 `set[T]` — `OriSet`

Prefix matches `OriList` so list len/get can operate on the dense prefix:

```c
typedef struct OriSet {
    int64_t *items;  /* dense [0..len) — same offset as OriList.data */
    int64_t  len;
    int64_t  cap;
    int64_t  version;
    int64_t *ht;
    int64_t  ht_cap;
    uint8_t  item_kind;
    void    *hash_fn; /* optional Hashable callback: i64(value) -> i64 */
    void    *eq_fn;   /* optional Equatable callback: (ptr,ptr) -> i8 */
} OriSet;
```

Generated native aggregate callbacks use compiler-private symbols
`__hash_helper_struct_<DefId>` and `__hash_helper_enum_<DefId>`. Their ABI is
`int64_t callback(void *payload)`. Structural non-recursive fields contribute
stable scalar/string/bytes hashes and nested aggregate hashes. A user-defined
`Hashable.hash(self) -> int` implementation, when present, is called by the
generated callback. When a type has an explicit non-structural `Equatable`
implementation without that method, the generated callback returns a constant
zero hash so the equality/hash invariant remains sound. Legacy equality-only
callers may leave `hash_fn` null; the runtime then keeps its linear
compatibility path.

### 7.6 `map[K,V]` — `OriMap`

```c
typedef struct OriMap {
    int64_t *keys;
    int64_t *values;
    int64_t  len;
    int64_t  cap;
    int64_t  version;
    int64_t *ht;
    int64_t  ht_cap;
    uint8_t  key_kind;
    void    *hash_fn;
    void    *eq_fn;
} OriMap;
```

Dense `keys[0..len)` / `values[0..len)`; hash table stores dense indices.

For a custom key, `hash_fn` is called before probing and `eq_fn` resolves
collisions. `ori_map_new_custom` and the `*_with_hash` variants install both
callbacks; the older equality-only entry points remain valid with a zero hash.

### 7.7 Other runtime structs

Additional `#[repr(C)]` payloads exist for concurrency and domain types
(`OriTaskJob`, `OriChannel`, `OriFuture`, `OriAtomicInt`, `RuntimeCancelToken`,
trees, graphs, deques, I/O streams, net sockets, …). Treat their field layouts
as **runtime-internal** unless a public Ori type documents a stable C view.
Opaque handles in the stdlib (e.g. `ori.fs.File`, `ori.net.Connection`) are
payload pointers; do not depend on their private layout from C without a
versioned export.

---

## 8. Symbol naming

### 8.1 Runtime / stdlib Layer 1

- Exact C names: `ori_<domain>_<op>` (examples: `ori_io_print`, `ori_list_new`,
  `ori_string_concat`, `ori_arc_retain`).
- Declared `#[no_mangle] extern "C"`.
- Manifest mapping lives in `ori-types` stdlib tables (`STDLIB_RUNTIME_FUNCTIONS`).

### 8.2 User Ori functions (native backend)

Qualified name `module.path.fn` (and nested paths) is mangled as:

```text
ORI__<escaped>
```

Escape rules (`mangle_symbol`):

| Character | Encoding |
|-----------|----------|
| ASCII alphanumeric or `_` | unchanged |
| `.` | `_dot_` |
| other | `_xNN_` where `NN` is lowercase hex of the code unit |

Examples:

| Ori name | Native symbol |
|----------|---------------|
| `app.main.foo` | `ORI__app_dot_main_dot_foo` |
| `ori.string.is_empty` (user code path) | `ORI__ori_dot_string_dot_is_empty` |

Globals:

```text
ORI_GLOBAL__<escaped>
```

Function-pointer thunks:

```text
ORI__<escaped>__fnptr_wrapper
```

**Historical note:** older drafts used `ori_MANGLE_<module>_<name>[_hash]`. That
scheme is **not** what the native backend emits. Generics are monomorphized to
distinct function names; there is no separate 16-char type hash suffix in the
current mangler.

### 8.3 Entry point

- If the program has an entry function named `main` (or `<module>.main` with
  empty params), the backend exports a C symbol **`main`** (Linkage::Export)
  that wraps the Ori main.
- That is the CRT entry for AOT executables and the JIT entry lookup target.

### 8.3a Hosted module lifecycle

A host must not call an exported Ori function before both runtime and module
initialization complete. It must use this order for each loaded generation:

1. call `ori_rt_init()` and require a zero result;
2. call `ori_rt_thread_attach()` on every foreign thread before that thread
   enters Ori;
3. call `__ori_module_init()` once (repeated calls are idempotent);
4. call exported functions;
5. call `__ori_module_shutdown()` to release managed globals;
6. call `ori_rt_thread_detach()` on every attached foreign thread;
7. call `ori_rt_shutdown_ex(timeout_ms)` and unload the runtime only after it
   returns zero.

`ori_rt_shutdown()` remains a compatibility wrapper with a five-second
deadline. Hosts that unload a library must use the result-returning form. Error
`ORI_HOST_ERROR_SHUTDOWN_BUSY` (`1006`) means executable code is still reachable
by a worker or a foreign thread remains attached; unloading in that state
violates the ABI.

Hosted JIT generations run their global initializer before publication and run
the paired teardown while their executable module is still alive. A failed
initializer is not published. Shared-module initialization uses a generated
guard and its shutdown resets that guard after clearing managed global slots.
Each slot is cleared before its value is released so destructor re-entry cannot
observe the value currently being finalized.

On Linux, runtime-created threads install a per-thread alternate signal stack.
Foreign threads use the attach/detach calls above. Runtime shutdown restores the
previous process `SIGSEGV`/`SIGBUS` actions before an allowed unload.

### 8.3b `@c_export` — the host-facing surface

`@c_export` on a `public` function emits an **unmangled** symbol with the
declared name, so a host can call or `dlsym` it directly. Scalar arguments use
their native representation; scalar-field structs cross through the
pointer/out bridge below; structs with managed fields cross as opaque ARC
handles. A custom symbol in `@c_export("name")` must be a portable,
non-keyword C/C++ identifier.

Accepted types:

| Ori type | C type |
|---|---|
| `int`, `int8`…`int64`, `u8`…`u64` | `int64_t`, `int8_t`…`int64_t`, `uint8_t`…`uint64_t` |
| `float`, `float32`, `float64` | `double`, `float`, `double` |
| `bool` | `bool` from `<stdbool.h>` |
| `string` | `const char *` — NUL-terminated |
| `bytes` | `const OriBytes *` — borrowed `{ data, len }` view; `len` is exact and may include `0x00` |
| scalar-field `struct` parameter | `const OriType *` |
| scalar-field `struct` return | `void` return plus final `OriType *out` parameter |
| managed `struct` parameter | borrowed `const OriTypeHandle *` |
| managed `struct` return | owned `OriTypeHandle *` |
| `optional[T]` parameter | `bool name_has_value, T name_value` |
| `optional[T]` return | `bool` plus `T *out` |
| `result[T, E]` parameter | `OriResultTag name_tag, T name_ok, E name_error` |
| `result[T, E]` return | `OriResultTag` plus `T *ok_out, E *error_out` |
| `void` (return only) | `void` |

`T` and `E` in the `optional`/`result` rows use the scalar, string, scalar
struct, bytes, or managed-handle bridge described in this section. A `result` branch
may be `void`, in which case its payload parameter is omitted; an `optional`
payload may not be `void`.

`bytes` parameters are copied into an ARC-managed allocation before the Ori
function is called, so the host may release or overwrite the input buffer after
the call returns. A `bytes` return uses `void` plus a final `OriBytes *out`
parameter; the host owns `out->data` and releases it with `ori_arc_release()`.

Direct `list`, `map`, `set`, and `tuple` values, plus nested
`optional`/`result`, generic structs, and empty structs, are **rejected** with
`attr.c_export_bad_type`. Collections may be stored inside an opaque managed
struct because their private layout never crosses the boundary.

#### Generated header (normative)

Every successful `ori compile --lib -o <library>` also writes a C header by
replacing the library extension with `.h`:

```text
libscores.so  → libscores.h
scores.dll    → scores.h
```

The header is the canonical host declaration. It contains:

- `<stdbool.h>` and `<stdint.h>`;
- `OriBytes { const uint8_t *data; int64_t len; }` when an export uses
  `bytes`; input views are borrowed for the call and output views transfer one
  owned ARC reference in `out->data`;
- `extern "C"` guards for C++;
- declarations for `ori_rt_init`, `ori_rt_version`, `ori_rt_abi_version`,
  `ori_rt_target`, thread attach/detach, `__ori_module_init`,
  `__ori_module_shutdown`, `ori_rt_shutdown_ex`, the compatibility
  `ori_rt_shutdown`, the hosted-error read functions, `ori_arc_retain`, and
  `ori_arc_release`;
- one complete `typedef struct` for each scalar struct used directly by an
  export and one incomplete handle declaration for each managed struct;
- `OriResultTag` with `ORI_RESULT_ERR = 0` and `ORI_RESULT_OK = 1` when an
  export uses `result`;
- all `@c_export` function declarations, including pointer/out lowering.

A unique source type `Point` is exposed as `OriPoint`. If two exported modules
contain the same short type name, the generator qualifies both names with
their module path. Field and parameter names that collide with C keywords are
suffixed with `_`; this does not change their order or ABI. Hosts should include
the generated header instead of duplicating declarations manually.

#### Scalar struct bridge (normative)

A non-generic, non-empty struct whose fields are only numeric scalars or
`bool` crosses the host boundary through a pointer. Its C layout uses field
declaration order, each field's natural C alignment, and tail padding to the
largest field alignment. This is independent of Ori's internal struct packing;
the export wrapper copies fields between the two layouts.

The host declares the corresponding C type:

```c
typedef struct {
    _Bool enabled;
    int64_t primary;
    int64_t fallback;
} OriPoint;
```

A parameter is a borrowed, non-null pointer. The wrapper creates an owned Ori
value before entering the function, so the host keeps ownership and may reuse
its struct after the call:

```c
int64_t choose_point(const OriPoint *point);
```

An Ori function returning the struct is exported as `void` with one final,
non-null output pointer:

```c
void make_point(_Bool enabled, int64_t primary, int64_t fallback, OriPoint *out);
```

The wrapper copies the returned fields into `out` and releases the temporary
Ori allocation before returning. No Ori allocation ownership crosses this
boundary. `@repr("C")` is not required because the wrapper performs the layout
translation explicitly.

#### Opaque managed struct handles (normative)

A non-generic, non-empty struct that is not eligible for the scalar bridge is
exported as an opaque handle. This includes structs containing `string`, nested
structs, collections, or other managed values. The header exposes only an
incomplete type:

```c
typedef struct OriProfileHandle OriProfileHandle;
```

The host cannot inspect or allocate this type. A valid handle is a non-null
pointer returned by an Ori export:

```c
OriProfileHandle *make_profile(const char *name, int64_t score);
int64_t profile_score(const OriProfileHandle *profile);
```

**Parameters are borrowed.** The export wrapper retains a registered handle
before entering the Ori function; the function consumes that temporary
reference through its normal scope cleanup. The host's reference remains valid
after the call.

**Returns are owned.** The host receives one ARC reference and must eventually
call `ori_arc_release(handle)`. If it deliberately creates another C-side owner,
it must first call `ori_arc_retain(handle)` and later release both references.
Passing an arbitrary foreign pointer as a handle is invalid: unlike a foreign
C string, it has no Ori object layout behind it. Generated wrappers validate
that each handle is a live allocation registered by the Ori runtime before
retaining it or entering user code. For a concrete non-generic opaque payload,
the compiler tags allocations and the wrapper validates the emitted payload
size and source-type tag. A null, foreign, wrong-size, or same-size wrong-type
pointer is rejected by the deterministic runtime bounds-failure path. Generic
or legacy allocations without a tag cannot cross a generated aggregate export
boundary; the native backend fails closed if it cannot emit the concrete
payload size and source-type tag required by the wrapper.

This is an additive host-facing contract. The struct's private field layout is
not part of `ori-native-abi-1`; hosts interact with it only through exported
functions.

#### Optional and result bridge (normative)

An `optional[T]` parameter is split into a presence flag and one payload:

```c
int64_t optional_score(bool score_has_value, int64_t score_value);
```

When `score_has_value` is false, the wrapper does not read a pointer-shaped
payload. An `optional[T]` return uses `bool` as the tag and appends one
non-null output pointer:

```c
bool find_score(const char *name, int64_t *out);
```

The wrapper writes `out` only when it returns `true`.

A `result[T, E]` uses the generated `OriResultTag`. Parameters provide the tag
and both possible payload positions; only the selected position is read:

```c
int64_t consume_result(
    OriResultTag value_tag,
    int64_t value_ok,
    const char *value_error
);
```

A result return uses the tag as its C return value and appends an output pointer
for each non-`void` branch:

```c
OriResultTag make_profile(
    const char *name,
    OriProfileHandle **ok_out,
    const char **error_out
);
```

Only the active branch is written. An active returned `string` or opaque handle
transfers one owned ARC reference to the host and must be released with
`ori_arc_release`; inactive output pointers are not dereferenced. Scalar struct
payloads use their natural-layout pointer/out bridge. Parameter payloads remain
borrowed under the same rules as their direct forms.

#### String ownership (normative)

An Ori string value *is* a pointer to NUL-terminated bytes; the ARC header sits
immediately **before** that pointer. Two rules follow, and they are not
symmetric:

**Parameters — the host keeps ownership.** The wrapper retains a registered
Ori string before the call and the function releases that temporary reference
on scope exit, so a string previously returned by Ori remains owned by the
host. `ori_arc_retain` / `ori_arc_release` are **no-ops** for memory Ori did not
allocate, so passing a literal, a stack buffer, or `malloc`'d memory is also
safe. The pointer must stay valid for the duration of the call.

**Returns — the host takes ownership.** The returned string is ARC-managed with
a reference count of 1 and no Ori scope owns it any more. The host must release
it:

```c
const char *s = shout("Ada");
puts(s);
ori_arc_release((void *) s);   /* exported by the runtime */
```

Failing to release **leaks; it does not dangle**. The pointer stays valid for as
long as the process lives, which is why forgetting the call is a slow bug rather
than a crash: 200 000 unreleased calls cost about 18 MB, the same workload with
`ori_arc_release` stays flat at process baseline.

A host that only reads the result immediately and never frees it is memory-safe
but unbounded. Treat `ori_arc_release` as mandatory in long-running hosts.

Regression tests: `check_c_export_accepts_string_params_and_return`,
`check_c_export_accepts_length_aware_bytes_params_and_return`,
`check_c_export_accepts_scalar_structs`,
`check_c_export_accepts_managed_struct_handles`,
`check_c_export_accepts_optional_and_result_bridges`,
`check_c_export_still_rejects_empty_generic_and_direct_collection_aggregates`,
`check_c_export_requires_a_portable_c_symbol_name`,
`compile_lib_c_export_produces_shared_object_on_linux`,
`compile_lib_c_export_scalar_struct_round_trips_through_a_c_host`,
`compile_lib_c_export_managed_struct_handle_preserves_host_ownership` (including
the foreign-pointer rejection path), and
`compile_lib_c_export_string_round_trips_through_a_c_host`, and
`compile_lib_c_export_optional_and_result_round_trip_through_c_host` (build real
C hosts with `cc`, include the generated headers, and exercise the full
ownership cycle).

### 8.4 Custom destructor callbacks

For each concrete struct or enum implementing `core.Destructor`, native codegen
emits a compiler-private `__ori_destructor_<DefId>` callback and passes its
address to `ori_alloc`. The callback invokes the user implementation's
`destroy(self)` method; it does not release stored managed fields.

At final release, the runtime removes the allocation from the ARC registry,
runs the callback while the payload and its fields are still readable, frees
the payload, and then releases surviving registered children. During cycle
collection, all callbacks run before any payload in that collected cycle is
freed. Registered edges remain the sole owner of managed fields.

---

## 9. Linking and packaging

| Artifact | Role |
|----------|------|
| `libori_runtime.a` (staticlib) | AOT link (`ori compile` / `ori test`) |
| `libori_runtime.so` / `.dll` / `.dylib` (cdylib) | JIT `ori run` symbol resolution |
| `runtime-link.json` | `target`, `runtime`, `ori_version`, **`abi_version`**, native static libs |

Link strategy priority (see [16-runtime-ffi-safety.md](16-runtime-ffi-safety.md)):

1. `ORI_NATIVE_LINKER` (raw)
2. `ORI_USE_BUNDLED_RUST_LLD=1`
3. `ORI_USE_SYSTEM_LINKER=1` / SystemLinker default path
4. `RustcDriver` fallback

Staging after runtime changes must refresh **both** staticlib and cdylib for
the host triple (stale cdylib → JIT UB).

---

## 10. Stability policy (pre-1.0)

Until Ori `1.0`, this ABI is **documented and versioned**, not forever frozen:

| Class | Change policy |
|-------|----------------|
| `ori-native-abi-1` layouts in §§4–7 | Breaking → new `ori-native-abi-N` + re-stage |
| New `ori_*` symbols | Additive OK without bump if old code never needed them |
| Mangling `ORI__*` | Breaking for tools that parse symbols → document + bump if tools depend |
| Opaque stdlib handles | Layout private; only constructor/destructor FFI is public |

Chapter [18-stability-and-compatibility.md](18-stability-and-compatibility.md)
lists language-surface stability separately from this binary contract.

---

## 11. Implementation map

| Concern | Primary location |
|---------|------------------|
| `ORI_ABI_VERSION`, ARC, collections | `compiler/crates/ori-runtime/src/lib.rs` |
| Layout math, mangling, main export | `compiler/crates/ori-codegen/src/native_backend.rs` |
| Driver ABI check + `runtime-link.json` | `compiler/crates/ori-driver/src/pipeline/runtime.rs` |
| Manifest symbol list | `ori-types` stdlib modules |
| FFI safety narrative | `docs/spec/16-runtime-ffi-safety.md` |
| ARC language rules | `docs/spec/10-memory.md` |

---

## 12. Checklist for ABI-touching PRs

- [ ] Layout or `ori_*` signature change justified and noted in CHANGELOG
- [ ] `ORI_ABI_VERSION` bumped if binary-incompatible with staged runtimes
- [ ] This chapter updated in the same PR
- [ ] Runtime re-staged (staticlib + cdylib) for CI/dev host
- [ ] Relevant `ori-driver` / runtime tests green
- [ ] Native AOT/JIT behavior follows the ABI contract

---

## History

| Date | Event |
|------|--------|
| pre-M3 | Draft chapter mixed aspirational C layouts with incorrect mangling/tag size |
| 2026-07-13 | **M3:** rewritten from runtime + native backend source of truth (`ori-native-abi-1`) |
