# Ori Language Specification — Chapter 19: Native ABI

> Status: **normative for the native backend** · **ABI-1 in force** (FREEZE-1 window)  
> Audience: compiler implementers, runtime maintainers, FFI authors  
> Surface: **S3** (`0.3.0`) + inference **`0.3.1`** + package **`0.3.2`**  
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

- C debug/transpile backend (partial parity; not the semantic reference).
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
| Runtime exports | `#[no_mangle] unsafe extern "C"` — C symbol = Rust `fn` name (`ori_list_new`, …) |

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
pointer. The hook is reserved for runtime-internal cleanup; cascading release
of stored managed children goes through registered ARC edges (single cascade
owner — see `docs/planning/adr-arc-single-cascade-owner.md` and Spec 10).
Layout guard test: `ori_heap_header_layout_is_stable` (`ori-runtime`).

### 6.2 Core ARC API (stable symbols)

| Symbol | Contract |
|--------|----------|
| `ori_alloc(size, destructor)` | Allocate header+payload; register; return payload; refcount = 1 |
| `ori_arc_retain(ptr)` | No-op if null or not registered |
| `ori_arc_release(ptr)` | No-op if null or not registered; free at 0 after dtor |
| `ori_arc_register_edge(owner, child)` | Strong edge owner→child for cycle GC; retains child |
| `ori_arc_unregister_edge(owner, child)` | Remove edge; release child edge retain |
| `ori_arc_collect_cycles()` | Trial-deletion collector; returns reclaimed count |

Detailed safety rules: [16-runtime-ffi-safety.md](16-runtime-ffi-safety.md).  
Language-level ARC rules: [10-memory.md](10-memory.md).

### 6.3 Registration

Only payloads returned by `ori_alloc` (or constructors that call it) are
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
} OriSet;
```

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

### 8.3b `@c_export` — the host-facing surface

`@c_export` on a `public` function emits an **unmangled** symbol with the
declared name, so a host can `dlsym` it directly. The wrapper forwards
arguments unchanged: the Ori representation of every accepted type is already
its C representation.

Accepted types:

| Ori type | C type |
|---|---|
| `int`, `int8`…`int64`, `u8`…`u64` | `long`, `int8_t`…`int64_t`, `uint8_t`…`uint64_t` |
| `float`, `float32`, `float64` | `double`, `float`, `double` |
| `bool` | `_Bool` |
| `string` | `const char *` — NUL-terminated |
| `void` (return only) | `void` |

Aggregates (`struct`, `list`, `map`, `set`, `optional`, `result`, `tuple`) are
**rejected** with `attr.c_export_bad_type`: they have no stable C layout in
ABI-1.

#### String ownership (normative)

An Ori string value *is* a pointer to NUL-terminated bytes; the ARC header sits
immediately **before** that pointer. Two rules follow, and they are not
symmetric:

**Parameters — the host keeps ownership.** Ori never frees a string it receives
from C. `ori_arc_retain` / `ori_arc_release` look the pointer up in the
allocation registry and are **no-ops** for memory Ori did not allocate, so
passing a literal, a stack buffer, or `malloc`'d memory is safe. The pointer
must stay valid for the duration of the call.

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
`check_c_export_still_rejects_aggregates`,
`compile_lib_c_export_string_round_trips_through_a_c_host` (builds a real C host
with `cc` and exercises the full cycle).

### 8.4 Destructors generated by codegen

Synthetic names such as `__dtor_struct_{id}`, `__dtor_enum_{id}`,
`__dtor_tuple_{n}` are compiler-private. They are registered as `ori_alloc`
destructor hooks for managed composite allocations.

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
| C debug backend | Not covered; may diverge |
| Opaque stdlib handles | Layout private; only constructor/destructor FFI is public |

Chapter [18-stability-and-compatibility.md](18-stability-and-compatibility.md)
lists language-surface stability separately from this binary contract.

---

## 11. Implementation map

| Concern | Primary location |
|---------|------------------|
| `ORI_ABI_VERSION`, ARC, collections | `compiler/crates/ori-runtime/src/lib.rs` |
| Layout math, mangling, main export | `compiler/crates/ori-codegen/src/native_backend.rs` |
| Driver ABI check + `runtime-link.json` | `compiler/crates/ori-driver/src/pipeline.rs` |
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
- [ ] No reliance on C debug backend for “truth”

---

## History

| Date | Event |
|------|--------|
| pre-M3 | Draft chapter mixed aspirational C layouts with incorrect mangling/tag size |
| 2026-07-13 | **M3:** rewritten from runtime + native backend source of truth (`ori-native-abi-1`) |
