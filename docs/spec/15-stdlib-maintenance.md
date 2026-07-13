# Stdlib maintenance flow

> Status: current as of 2026-06-29.
> Audience: stdlib implementers, compiler implementers
> See also: [`12-stdlib.md`](12-stdlib.md) — Implementation Architecture (v1.x)

Goal: adding a stdlib function should not require guessing four different places.

## Source of truth

The v1.x stdlib is implemented as a Rust manifest plus a native runtime, not
as separate `.orl` source modules. `compiler/crates/ori-types/src/stdlib.rs`
is the single source of truth for the stdlib contract surface.

Main entries:

- `STDLIB_RUNTIME_FUNCTIONS`: canonical path, aliases, runtime symbol and backend flags.
- `STDLIB_MODULE_ONLY_PATHS`: importable `ori.*` modules without runtime entries
  (`ori`, `ori.core`, `ori.Error`, `ori.mem` (inline intrinsics), `ori.concurrent`
  (umbrella)).
- `stdlib_func_sig`: semantic type signature used by typecheck and HIR lowering.
- `stdlib_native_abi`: native ABI used by the Cranelift backend.
- `is_implemented_stdlib_module` / `implemented_stdlib_modules`: the importable
  module set, derived from the manifest plus `STDLIB_MODULE_ONLY_PATHS`.
- `stdlib_runtime_symbol`: canonical-path → runtime symbol lookup (used by
  `ori-hir::lower::stdlib_c_name` and the C backend).

Downstream crates must not keep parallel hardcoded lists. The driver
(`pipeline::classify_stdlib_import`, `pipeline::append_stdlib_documentation`)
and HIR lowering (`lower::stdlib_c_name`) delegate to the manifest.

## Add a new runtime-backed function

- [ ] Add the canonical `ori.module.name` path to `STDLIB_RUNTIME_FUNCTIONS`.
- [ ] Add aliases only when older code or ergonomic short names need them.
- [ ] Set `native_runtime: true` through the `stdlib!` macro.
- [ ] Set `c_backend_runtime: true` only when the C/debug backend really supports the call.
- [ ] Add the semantic signature in `stdlib_func_sig`.
- [ ] Add the native ABI in `stdlib_native_abi`.
- [ ] Export the runtime symbol from `compiler/crates/ori-runtime/src/lib.rs`.
- [ ] Declare or route the symbol in `compiler/crates/ori-codegen/src/native_backend.rs` only when the generic manifest path is not enough.
- [ ] Add one typecheck test.
- [ ] Add one native execution test.
- [ ] Add a C/debug test only if `c_backend_runtime` is true.

## Add a pure compiler intrinsic

- [ ] Do not add it to `STDLIB_RUNTIME_FUNCTIONS` unless it calls a runtime symbol.
- [ ] If it owns an importable module with no other runtime entries, add the
      module path to `STDLIB_MODULE_ONLY_PATHS` (e.g. `ori.mem`).
- [ ] Keep type rules in `stdlib_func_sig` or the typechecker helper that owns that family.
- [ ] Keep lowering in HIR or codegen, not both.
- [ ] Add a test proving the path does not need a runtime symbol.

## Drift checks

These tests must stay green (in `ori-types::stdlib::tests` and
`ori-driver::pipeline::tests`):

Manifest integrity:

- `manifest_paths_and_aliases_are_unique`
- `manifest_resolves_aliases_to_runtime_symbols`
- `manifest_runtime_entries_have_type_and_native_abi_metadata`
- `stdlib_manifest_paths_lower_to_declared_runtime_symbols`
- `native_backend_declares_manifest_runtime_symbols`

Module classification (Etapa 8.1 consolidation):

- `manifest_module_prefixes_are_all_implemented`
- `implemented_stdlib_modules_covers_legacy_hardcoded_list`
- `unknown_stdlib_modules_are_rejected`
- `collection_stdlib_doc_signatures_reference_implemented_modules` (pipeline)

Spec parity:

- `spec_c_backend_matrix_matches_manifest_flags`
- `spec_fs_and_json_contracts_match_stdlib_sig`

If one fails, fix the manifest instead of patching the failing backend locally.

## `.orl` source modules (Stdlib Phase 0+)

As of Stdlib Phase 0 (unreleased, `[Unreleased]` cycle), the stdlib supports
`.orl` source modules (Layer 2) that sit alongside the Rust manifest (Layer 1).
This is no longer a future item — the infrastructure is live.

### Architecture

- **Layer 1 (Rust runtime):** manifest-only, `extern "C"` FFI — never ported.
  Hot-path modules (collections, async, I/O, ARC, string primitives like
  `concat`/`slice` that allocate) stay here.
- **Layer 2 (`.orl` wrappers):** `stdlib/**/*.orl`, call Layer 1 via normal
  `import`. Cold compositional functions go here. Modules include
  `ori.validate`, `ori.path`, `ori.string`, `ori.list`,
  `ori.convert`, `ori.map`, `ori.set`, `ori.bytes`,
  `ori.math`, `ori.json`, `ori.io`, `ori.fs`,
  `ori.time`, `ori.test`, `ori.process`,
  `ori.concurrent`. Gap parity map: `docs/planning/stdlib-gap-parity.md`.
- **Layer 3 (`.orl` algorithms):** pure-Ori algorithms on top of Layer 1+2.
  Modules: `ori.list` (`sum_int`, `binary_search_int`,
  `all_equal_int`), `ori.tree` (iterative traversals:
  `values_preorder`, `leaf_count`, `max_depth_from`, `is_leaf`),
  `ori.graph` (BFS in `.orl`: `has_path`, `reachable_count`,
  `is_reachable`, `has_path_int`). Layer 3 avoids recursive generic helpers
  (typechecker rejects `generic.circular_instantiation`) and runtime traversal
  shortcuts where the goal is to express logic in Ori itself.

### Path convention

`ori.X.Y.Z` -> `stdlib/X/Y/Z.orl` (dots become directory separators, file
extension is `.orl`). The file must declare `namespace ori.X.Y.Z` (enforced
by `validate_import_namespace`).

### Discovery

`pipeline::classify_stdlib_import` keeps runtime modules lightweight for normal
imports. If `ori.string`, `ori.list`, or `ori.fs` is imported with an alias, the
runtime manifest wins and no parent `.orl` helper module is loaded.

When the import has selected items, for example
`import ori.string only (is_empty)`, the driver may load the matching parent
`.orl` file with `StdlibSource(PathBuf)`. Unknown `ori.*` modules still check
`find_stdlib_source_module` before returning `Unknown`.

Loaded stdlib source files use the same `load_source_recursive` path as user
files, including cycle detection and namespace validation.

### Stdlib root resolution

`find_stdlib_root` resolves in order:
1. `ORI_STDLIB_ROOT` env var (override for tests/packaging)
2. `CARGO_MANIFEST_DIR/../../../stdlib` (dev mode — workspace stdlib dir)
3. `<ori.exe dir>/stdlib` (release package layout)

### Visibility

Functions in `.orl` stdlib modules must be declared `pub` to be callable
from other namespaces — same rule as user code. Private functions are only
visible within the same namespace (useful for internal helpers in a stdlib
module).

### Adding a Layer 2 function

- [ ] Create or extend `stdlib/<module path>.orl` matching the namespace.
- [ ] `import ori.<layer1_module> as <alias>` to access Layer 1 primitives.
- [ ] Declare functions with `pub func ...`.
- [ ] Avoid Ori keywords as identifiers (`string`, `repeat`, `result`, etc.
  are reserved — use `str`, `replicate`, `acc` or similar alternatives).
- [ ] Avoid local variable names that collide with runtime internal symbols.
  The native backend declares `ori_len` (`ptr: *u8) -> i64` and inserts it
  into `stdlib_ids` — a local variable named `len` gets mangled to `ori_len`
  and conflicts with the runtime symbol, producing `undefined variable
  'ori_len' in native codegen` at compile time. Use a qualified name like
  `s_len`, `sub_len`, `total_len` instead.
- [ ] Prefer indexed iteration over `for item in list<string>` in Layer 2/3
  stdlib (ARC loop binding still fragile for managed string elements).
- [ ] Map/set/graph modules: use concrete key types (`string`, `int`) until
  the `Hashable` + `Equatable` trait gate supports generic keys.
- [ ] Add a regression test in `multifile_imports.rs` that imports the module
      and validates behavior end-to-end (check -> compile -> run).

No manifest changes are needed — the compiler discovers Layer 2 modules by
scanning `stdlib/` at compile time.

## Current cleanup left

- Reduce old fallback helpers in `compiler/crates/ori-types/src/check.rs`.
- Keep compatibility aliases only when a test or public doc needs them.
- Prefer one shared table over duplicating signatures in typecheck, HIR and codegen.

## Namespace flattening (Opção C)

Status: partially implemented for `ori.string`, `ori.list`, and `ori.fs`.
The old submodules remain compatible.

For usability and ergonomics, parent modules may merge Layer 2
(compositional) and Layer 3 (algorithm) helpers directly into the parent Layer
1 module namespace.

Implemented parent modules:

- `stdlib/string.orl` -> `ori.string`
- `stdlib/list.orl` -> `ori.list`
- `stdlib/fs.orl` -> `ori.fs`

Instead of importing many helper modules:
```lua
import ori.string as str
import ori.string as su
import ori.string as sa
```
Users can import only the helper names needed from the unified root namespace:

```lua
import ori.string only (is_empty, truncate as cut)
```

Normal alias imports such as `import ori.string as str` continue to expose the
native runtime surface (`str.len`, `str.slice`, `str.parse_int`, etc.) without
forcing the parent `.orl` helper module into every compile.

Compatibility rule: old submodules remain valid. `ori.string`,
`ori.string`, `ori.list`, `ori.list`, and
`ori.fs` are still loaded from their existing files. Do not remove them
until a future breaking release has a documented migration window.
