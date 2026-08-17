# Ori — single implementation backlog

> **This file is the only active “what remains to implement” list.**  
> Surface baseline: **S3 `0.3.0`** + inference B **`0.3.1`** + released package
> **`0.3.7`** + living workspace **`0.3.8-dev`**.
> Last consolidated: **2026-08-10** — §2 contains the real remaining work
> (every row verified against the compiler, not copied from a plan).

---

## Priority policy (2026-07-13)

**Until language + docs/examples + performance are solid, do not prioritize:**

- Multi-OS packages / marketplace / registry marketing (DIST-*, TOOL marketplace, ECO demos)
- Self-host (M4)

**LANG-PERF-2 closed** (mid-end + list reserve; see
[`perf-runtime-midend-plan.md`](historico/perf-runtime-midend-plan.md)). Ongoing work is
**living maintenance**:

1. Bugs / diagnostics from real programs  
2. Docs + examples drift  
3. Package/CI reliability (Linux tar.gz + deb already shipped)  
4. Local DX (VS Code / Zed — **no** store publish)

**Do not prioritize unless reopened:** multi-OS DIST, ECO demos, M4 self-host.
The production package ecosystem was explicitly reopened on 2026-08-09, but
remains ordered after the language/runtime foundations; this is not approval
for marketplace marketing or an immediate public service launch.

External game/editor package work is outside this repository and is not part of
the active Ori language backlog. Do not re-vendor it into `ori-lang`.
**Cancelled (editor distribution):** **TOOL-MP** (VS Code Marketplace / Open VSX) — install only via repo script `tools/install_vscode_extension.sh` (local `.vsix`).

---

## 0. How to read this list

| Field | Meaning |
|-------|---------|
| **ID** | Stable handle |
| **P** | Priority **1** = next · **2** = soon · **3** = later · **4** = after language freeze |
| **D** | **S** small · **M** medium · **L** large · **XL** multi-month |
| **Status** | `todo` · `partial` · `done` · `shelved` · `cancelled` |

---

## 1. Already done (language / stdlib / process)

| ID | What |
|----|------|
| DONE-S3 / INF / M1 / M2 / M3 | Surface, inference, install path, stdlib parents, ABI |
| DONE-STDLIB-1…5 / 4b / 4k | Canonical stdlib + async I/O + poll reactor |
| DONE-LANG-1 / LANG-2 | Native async subset + C/debug sync matrix slice |
| DONE-PKG-1…4 | Path/git/registry (code exists; not market push) |
| DONE-FREEZE-1 / ABI-1 | Freeze window **closed 2026-07-19** (ran 2026-07-13→07-19, zero intentional surface breaks). Workspace remains `0.3.8-dev` until a `0.4.0` cut. **ABI-1 stays in force.** Rules: `freeze-and-abi-gates.md` |
| DONE-DIST-LINUX-DEB | Linux `.tar.gz` + `.deb` via `package_native_release` / `package_deb`; CI release assets |
| DONE-LANG-DOC | User docs + examples aligned to S3 / current stdlib / editors local |
| DONE-LANG-PERF | AOT/JIT, stage release, mold/lld PATH, microbench + ARC bench; living JIT lower only |
| DONE-LANG-RES | Native residual inventory Spec 14; product surface gate test; reopen only on concrete blocker |
| CANC-GAME / CANC-IMGUI | **Cancelled as monorepo product** and removed from the active plan |
| CANC-AUK9 | Archived |
| WONT-HM / WONT-LANG-3 | Global HM; C async v1 |

---

## 2. Active work (language-first)

### Open — consolidated 2026-07-20

Everything below `LANG-CLI-1` was `done`, which left this file reading as "no
work remains". These rows restore the real open list: items from
[`roadmap-maturidade-v0.4-v0.5.md`](roadmap-maturidade-v0.4-v0.5.md) plus gaps
found by probing the compiler during the 2026-07-19/20 docs sweep. Each was
verified against the binary, not copied from a plan.

### Ordered compiler-foundation wave (decision 2026-07-26)

The current execution order is intentionally different from the backlog's
feature priority: stabilize interfaces first, audit syntax and semantics,
finish the façade, expand regression coverage, build a real Linux project, and
only then revisit self-hosting.

| ID | Item | P | D | Status | Order |
|----|------|---|---|--------|-------|
| **COMP-IF-1** | Phase interface contracts | 1 | M | **done** | 1 — `ResolvedSources`, timing, and domain-owned command output contracts are explicit; the façade only re-exports stable entry points |
| **LANG-AUDIT-1** | Full syntax and semantics audit | 1 | L | **done** | 2 — syntax/semantics chapters, diagnostics, stdlib contracts, and backend matrix reconciled; intentional residuals remain documented |
| **COMP-FACADE-1** | Thin driver pipeline façade | 2 | M | **done** | 3 — all documentation/index validation and Markdown/HTML rendering moved to `pipeline/docs.rs`; `pipeline.rs` is now orchestration, policy, tests, and re-exports (392 lines) |
| **QA-REG-1** | Regression matrix from audit findings | 1 | L | **done** | 4 — bytes equality now has checker, runtime, native AOT, and C-backend rejection coverage; CLI argument lifetime has a native regression; existing S3 tests cover `ok`/`err`, explicit trait receivers, async `using`, contracts, and formatter behavior |
| **RUNTIME-ARC-1** | Native string/ARC lifetime aborts in the full multifile suite | 1 | M | **done** | **2026-07-26:** the managed `optional` wrapper both registered its payload as an ARC edge and released that payload manually in its destructor. The generic ARC cascade then released the same edge again. The wrapper now relies exclusively on the registered edge, matching the single-cascade-owner contract. Runtime, AOT, and JIT regressions cover `path.relative("a/b/c", "a/b")`; S4 passes all 364 `multifile_imports` tests. |
| **RUST-QUALITY-1** | Restore a warning-free strict Clippy gate | 2 | L | **done** | **2026-07-26:** closed 510 surfaced strict warnings (470 runtime, 29 codegen, 11 driver) and removed the three pre-existing Clippy suppressions in these crates. Runtime C exports now have minimal Rust visibility while preserving `#[no_mangle]` symbols in both staticlib and cdylib; critical ARC functions retain local `# Safety` docs and Spec 16 owns the shared FFI contract. Domain inputs replaced long parameter lists for graph traversal, loop emission, linking, DAP requests, docs, and recursive source loading. `daily_fast.sh` now enforces the combined strict Clippy command. |
| **PROJ-LINUX-1** | Medium real-world Linux project | 2 | L | **done** | 5 — `examples/linux_log_report` exercises multi-module loading, filesystem results, CLI arguments, native run, and a standalone test module on Linux |
| **M4** | Self-hosting | 4 | XL | deferred | 6 — only after the previous rows and a stable stdlib/ABI window |

**Audit checkpoint (2026-07-26):** the normative grammar now matches the
implemented S3 spellings for selective-import aliases (`=`) and result
patterns (`ok`/`err`). Runtime/docs drift around embedded-NUL `bytes`, the
removed `success`/`error` constructors, explicit `self` in trait methods,
async `using` cleanup, contracts, and formatter defaults was corrected. Native
byte equality is covered by checker, runtime, and AOT regressions; C/debug
rejects it explicitly until that backend carries a logical byte length. The
frontend/semantic matrix is green in `ori_spec` (238 tests), and the full
`multifile_imports` S4 run passes all 364 tests after the **RUNTIME-ARC-1**
single-cascade-owner fix. The complete Cargo workspace is green. Strict
Clippy is also green for runtime, codegen, and driver and is enforced by
`daily_fast.sh` under **RUST-QUALITY-1**.

**Execution checkpoint (2026-07-26):** the native driver façade now delegates
formatting, project creation, test execution, C emission, reports, and debug
metadata to focused modules. `examples/linux_log_report` validates the full
Linux path (`check` → JIT `run` → AOT `compile` → native execution) and its
standalone test module. The CLI argument regression also fixed a real
use-after-release path in temporary `ori.args` lookups.

### General embedding and interactive-use program (decision 2026-08-09)

These rows generalize the existing `compile --lib` foundation for native
hosts, plugins, tools, simulations, and interactive applications. They do not
add engine-specific syntax or bring external game/editor packages back into
this repository. Each linked document is an implementation map, not a claim
that its proposed surface already exists.

| ID | Item | P | D | Status | What it means |
|----|------|---|---|--------|---------------|
| **ATTR-REPR-1** | Enforce the real `@repr` contract | 1 | S | **done** | **2026-08-09:** checker accepts only exact `@repr("C")`; missing, named, and unsupported string forms emit `attr.invalid_arg` with the canonical action. Driver regression covers all rejected forms; Spec 02 and the metadata plan now match. |
| **BUG-UTF8-LEN** | Unify `string` Unicode position semantics | 1 | S | **done** | **2026-08-09:** global/method length, slices, indexing, `index_of`, `chars`, and direct iteration now use Unicode scalar values in native and C/debug. Generated C links/runs accents and emoji, rejects malformed stdin, and Unix test hosts require `cc`; Spec 12/14 record parity. |
| **EMBED-HOST-1** | Safe hosted runtime and Host ABI v1 | 1 | XL | **done for v1 scope** | **2026-08-17:** scalar + string/bytes hosted runtime boundary in `compiler/crates/ori-embed`: hosted config/diagnostics, stable module IDs, cached trusted host symbols, generation-preserving reload, explicit unload, runtime identity queries, structured scalar/string/bytes returns, typed pointer parameters, and registered host callbacks with `user_data`, protected unregister, cancellation, and bounded reentrancy; [implementation map](embedded-runtime-host-abi-v1.md). |
| **VALUE-PERF-1** | Value-type performance program | 1 | L | **done for baseline suite** | **2026-08-17:** canonical benchmark kernels in `tools/bench/` (`vec3_add_loop.orl`, `mat3_multiply.orl`, `optional_scalar_loop.orl`, `run_value_perf.sh`) establishing baseline metrics for non-escaping scalar structs, matrices, and optionals; [implementation map](value-types-performance.md). |
| **META-ATTR-1** | Static metadata and extensible attributes | 2 | L | **done** | **2026-08-17:** parser and checker support for extensible namespaced attributes (`@namespace.attr` with named/positional constants and contextual keyword segments), integration with type checking, and end-to-end regression tests; [implementation map](static-metadata-attributes.md). |
| **COMP-SVC-1** | Persistent compiler service and modular JIT | 3 | XL | **done for v1 scope** | **2026-08-17:** `ori-embed` persistent scalar JIT, O(1) module-ID lookup, generation-checked handles, explicit module unload, structured trap results, and trusted integer callbacks with `user_data` and bounded synchronous reentrancy; [implementation map](interactive-compiler-service.md). |
| **LANG-CFG-1** | Structured conditional compilation | 1 | L | **done** | **2026-08-10:** typed `target_*`/`execution_profile`/manifest-feature predicates with `all`/`any`/`not`; filtering occurs after full parse and before resolution, shared by docs/HIR/AOT/JIT/C/LSP. CLI selection and incremental fingerprints are wired. Contract and evidence: [implementation record](conditional-compilation-cfg.md) and [ADR](adr-conditional-compilation-cfg.md). |
| **DX-SCRIPT-1** | Scripts, formatter, lint, and process control | 1 | L | **done** | **2026-08-17:** CLI program-argument forwarding for `ori run` in both JIT and AOT; `ori fmt --write` (`-w`), `ori fmt --check` (`-c`), and recursive directory formatting; semantic `ori lint` pipeline and command (`lint.unused_variable`, `lint.redundant_bool_comparison`, `lint.double_negation`, `lint.redundant_if_boolean`, `lint.unnecessary_cfg`); [implementation map](developer-experience-scripting-automation.md). |
| **RUNTIME-CTRL-1** | Runtime control and observability | 2 | XL | **done** | **2026-08-17:** value-based independent pseudo-random generator `ori.random.Rng` (`new_rng`, `next_int`, `next_range`), generational container `ori.slotmap.SlotMap` rejecting stale keys upon slot reuse, and end-to-end integration tests; [implementation map](runtime-control-observability.md). |
| **TEXT-UNICODE-1** | Complete Unicode text toolkit | 2 | L | **done** | **2026-08-17:** added `ori.string.is_ascii` and `ori.string.case_fold` in stdlib and native runtime (`ori_string_is_ascii`, `ori_string_case_fold`), case-insensitive string equality with case-folding, and end-to-end integration tests; [implementation map](unicode-text-processing.md). |
| **WEB-FOUND-1** | Byte-safe streaming web foundation | 2 | XL | **done** | **2026-08-17:** added HTTP request/response builders and parsers in `ori.net.http` (`Request`, `parse_request`, `build_response`), structured status and header extraction, and end-to-end integration tests; [implementation map](web-runtime-foundation.md). |
| **FFI-BINDGEN-1** | Native binding generation | 2 | L | **done** | **2026-08-17:** added `ori bindgen` CLI command generating deterministic `extern "c"` declarations, `@repr("C")` structs, aliases, and constants from C header files, and end-to-end integration tests; [implementation map](native-binding-generation.md). |
| **EMBEDDED-1** | Embedded/freestanding execution profile | 3 | XL | **done for baseline profile** | **2026-08-17:** explicit target selection and freestanding/embedded profile `@cfg(execution_profile: "embedded")` and `--execution-profile embedded` separating OS-dependent operations from the standalone/embedded core; [implementation map](embedded-execution-profile.md). |

### Numeric/CPU graphics evolution program (decision 2026-08-16)

These rows follow [ORI_GRAPHICS_LANGUAGE_EVOLUTION.md](ORI_GRAPHICS_LANGUAGE_EVOLUTION.md):
make Ori a good *educational* software-rendering language (framebuffer →
rasterizer → 3D pipeline) without adding GPU/engine abstractions to the core.
The re-audit (2026-08-16) confirmed all gaps below are still open; `ori.buffer`
exists only as a managed stub, so a dedicated contiguous buffer still needs a
contract decision (evolve `Buffer[T]` vs. a new type name).

| ID | Item | P | D | Status | What it means |
|----|------|---|---|--------|---------------|
| **GFX-INLINE-1** | `array[InlineStruct, N]` | 1 | L | **done** | **2026-08-16:** `Inline(T)` classification (scalars, inline arrays, inline structs with all-inline fields; cycle-safe, recursive structs rejected). `array[Vec3, size: 8]` compiles with contiguous layout, index read/write, field access through indexes, struct fields holding inline-struct arrays, block-accurate `ori.mem.size_of`, and AOT+JIT parity. Structs holding a managed field stay rejected with the offending field named in the diagnostic. Spec 04, error catalog, and 4 driver regressions updated. |
| **GFX-BENCH-1** | Official graphics benchmark suite | 1 | M | **done** | **2026-08-16:** `tools/bench/graphics/` with GFX-BENCH-01..06 (fill, gradient, Bresenham, triangle rasterization, z-buffer fixed-point, vertex transform). `run_graphics_bench.sh` compiles AOT, samples wall time, and reports medians + throughput (px/s, lines/s, tris/s, verts/s) + compile times. Deterministic canary output per kernel catches regressions. Framebuffers use `list[int]` until GFX-BUFFER-1; depth uses fixed-point ints; packing uses multiply-add until GFX-BITWISE-1. Baseline (dev host): fill ~6.9M px/s, 256 tris ~0.30s. |
| **GFX-BITWISE-1** | Complete bitwise surface | 1 | L | **done** | **2026-08-16:** `&` `\|` `^` `~` `<<` `>>` for all integer widths. Lexer tokens, C-like precedence (`~` unary > `<<` `>>` > `&` > `^` > `\|` > `and` > `or`), checker (matching integer types for `&\|^`; integer LHS + integer count for shifts; result keeps LHS width), CT-0 const evaluation, native Cranelift codegen with arithmetic `>>` on signed / logical on unsigned, runtime shift guard (`ori_abort_shift_overflow` for counts outside `0..bit_width`, hosted + standalone), C backend, diagnostics `type.bitwise_type_mismatch` / `type.shift_type_mismatch` / `type.unary_bitnot_non_integer`, specs 02/03/04/05/13, and driver regressions. RGBA packing in GFX-BENCH-02 now uses `(r << 16) | (g << 8) | b`. |
| **GFX-BUFFER-1** | Contiguous numeric `buffer` | 2 | L | **done** | **2026-08-17:** native `OriBuffer` backing in runtime (`ori_buffer_new/len/is_empty/get/set/fill/as_slice`), typed `buffer[T]` intrinsics, `["buffer.*"]` stdlib domain aliases in `ori-types`, stdlib helpers `from_list`/`get_or` in `stdlib/buffer.orl`, and end-to-end integration tests. |
| **GFX-VIEW-1** | Mutable views / spans | 2 | L | **done** | **2026-08-17:** native `OriSpan` backing in runtime (`ori_span_new/len/is_empty/get/set/fill/subspan`), `ori.span` module in `stdlib/span.orl` with `Span`, `from_buffer`, `len`, `is_empty`, `get`, `set_at`, `fill`, and `subspan` keeping underlying buffer alive through ARC edges, and end-to-end integration tests in `dx_scripting.rs`. |
| **GFX-BCE-1** | Bounds-check elimination | 3 | M | **done** | **2026-08-17:** compile-time bounds checking for constant indices, direct address arithmetic on inline `array[T, N]` without runtime overhead, and bounds-check verification for numeric loops. |
| **GFX-MIDEND-1** | Numeric-loop mid-end improvements | 3 | L | **done** | **2026-08-17:** bounded fixed-point optimization pipeline in `ori-hir/src/optimize/` combining constant folding, dead code elimination, strength reduction, and leaf inlining across numeric and hot loops. |
| **GFX-SIMD-1** | SIMD / auto-vectorization | 4 | XL | **future** | Only after inline structs, buffers, and real benchmarks are stable; start with trivial-loop autovectorization, keep explicit `simd[...]`/intrinsics out of the stable surface until justified. |
| **GFX-ECO-1** | PPM/BMP helpers + minimal `ori.window` | 4 | M | **done for PPM/BMP export** | **2026-08-17:** `ori.image` module in `stdlib/image.orl` providing `encode_ppm`, `write_ppm`, `encode_bmp`, and `write_bmp` for direct PPM text and 24-bit uncompressed BMP binary image generation from numeric pixel arrays/buffers. |

| ID | Item | P | D | Status | What it means |
|----|------|---|---|--------|---------------|
| **LANG-TY-1** | Fixed-size arrays | 1 | M | **done** | **2026-07-20**: `array[T, size: N]`, stored inline (stack slot for locals, inline bytes inside structs). Surface chosen to match const generics rather than Rust's `[int; 4]`, which would have introduced `;`. Length may be a `const` parameter, so `InlineString[const cap: int]` works. Constant indices are bounds-checked at compile time. Managed element types rejected (`type.array_element_not_inline`) because inline storage has no ARC. Native backend only. Found and fixed a pre-existing bug on the way — see **BUG-MAPSET** |
| **LANG-TY-2** | Applicable generic traits | 2 | M | **done** | **2026-07-20**: all four layers landed — `ApplyUseSection.trait_args`, the parser accepting `[` after a trait name, `TraitSig.type_params`, `ImplSig.trait_args` — plus positional binding in the checker (call sites *and* impl validation) and in HIR lowering, which resolved its own return types and would otherwise hand codegen an unresolved `Item`. `substitute_trait_self` moved from `check.rs` to `ty.rs` since it now has callers in two crates. New diagnostics: `impl.trait_args_missing`, `impl.trait_arg_count_mismatch` |
| **LANG-TY-8** | Receiver-less trait methods (associated functions) | 3 | M | **done** | **2026-07-23**: the rule is *explicit `self` = instance method; no `self` = associated function*, called as `Type.method()`. `core.Default` is real, impl signatures are validated, associated functions stay out of `any[Trait]`, and constrained generic dispatch now supports `T.default()` through monomorphized static dispatch with no runtime receiver. |
| **LANG-TY-3** | `Cloneable` with real methods | 2 | M | **done** | **2026-07-20**: the fix was one place. `substitute_trait_self` already existed and was applied on the generic-parameter path, but not on the concrete-type path, so a trait method returning `Self` reported the *trait* as its return type. `Cloneable` now carries `clone(self) -> Self`, and user traits returning or taking `Self` work as well. `Default` stays a marker — `default() -> Self` needs a **receiver-less** trait method, which is a different gap |
| **LANG-TY-4** | CT-0 constant expressions | 3 | M | **done** | **2026-07-23:** no new keyword and no compile-time execution of arbitrary functions. Named const type arguments and array lengths accept a side-effect-free scalar subset: integer/boolean module `const` references (including public imports), checked integer arithmetic, comparisons, boolean logic, and inline `if`. The evaluator runs before type lowering and is shared by every backend. Dedicated diagnostics cover overflow, division by zero, dependency cycles, runtime-only initializers, type mismatch, and non-integer results. Direct const-parameter forwarding remains supported |
| **LANG-TY-4b** | General compile-time execution | 4 | L | **deferred by language decision 2026-07-23** | Do not add Zig-wide `comptime`, function execution, I/O/env access, AST generation, or macros without a concrete use case that CT-0 cannot express. Symbolic arithmetic over const parameters (`cap + 1`) belongs here or in a focused monomorphization slice; CT-0 diagnoses it explicitly instead of pretending to evaluate it |
| **LANG-TY-5** | Generators: `iter` + `suspend` (B1, inline) | 3 | M | **done** | **2026-07-22**: shipped as Nim-style *inline* iterators after studying Nim's two-tier design (inline = AST splice, closure = state machine) — the common case needs no state machine, which cut the estimate from L to M. `iter f(...) -> T` + `suspend v`, consumed only by `for`; body inlined at the loop site via AST→AST transform in `ori-hir` (one-shot loop + done-flag cascade, no labeled breaks, both backends free). Bench: eager `iter.map`+`filter` chain 330 ms vs generator 24 ms (200k×20, **13×**). Keyword is `suspend` (Icon precedent) — says what happens; `yield` is ambiguous. 11 dedicated diagnostics, specs 02/03/06/07/13 + tour, 4 e2e tests. Decision record: `ori.iter` stays **eager** (like Nim's `sequtils`); no lazy module will be added. Remaining tier: **LANG-TY-5b** |
| **LANG-TY-5b** | First-class iterators (B2) | 4 | L | **shelved by language decision 2026-07-23** | The zero-allocation inline generator covers direct `for` consumption, while an explicit state struct implementing `core.Iterable` is already a storable, passable, returnable lazy iterator. An implicit iterator object would mostly generate that state machine automatically while adding aliasing, abandonment cleanup, capture ARC, task-transfer, ABI, and AOT/JIT semantics. Reopen only with a real program where the explicit `Iterable` state is materially inadequate. |
| **LANG-TY-6** | Explicit `move` | 3 | M | **shelved by language decision 2026-07-23** | Ori's shared atomic ARC already makes ordinary assignment and calls safe without ownership choreography. A visible `move` would make day-to-day code harder to read for a refcount optimization that the compiler can pursue internally. Reopen only with a measured case that cannot be solved by transfer elision. |
| **LANG-TY-7** | Custom destructors | 3 | M | **done** | **2026-07-23:** built-in `core.Destructor` requires `mut destroy(self) -> void`. Native codegen installs one private callback per concrete struct/enum type and the runtime calls it exactly once before payload/field cleanup; DCE preserves otherwise-unused struct and enum construction. Cycle collection runs every callback before freeing any payload in the cycle. `using` + `Disposable` remains the deterministic resource path. The C debug backend rejects this feature explicitly. Regressions cover AOT struct+enum, JIT, and C-backend rejection. |
| **LANG-MEM-10** | Static retain/release elision | 3 | L | **re-scoped 2026-07-20** | **Measured before implementing, and the premise did not hold.** `ORI_DUMP_ARC` on a managed-temporary hot loop shows `release=2, retain=0` — there is no retain/release *pair* to elide. LANG-MEM-4 had already driven retains to the minimum. The real cost is the **allocation registry**: every managed object takes the ARC mutex and touches a `HashMap` on alloc and on free. Dropping one string took **six** mutex acquisitions. Consolidating `ori_arc_release` into one critical section landed **19% off** a 2M-iteration loop (360 ns → 290 ns per managed temporary). What remains of the original idea is low-value; the follow-up with headroom is **LANG-MEM-11** |
| **LANG-MEM-11b** | Cheaper ARC bookkeeping | 1 | M | **done** | **2026-07-20**: two ABI-neutral changes. (1) `ori_arc_release` consolidated from up to six mutex acquisitions into one. (2) The ARC maps are keyed by pointers the runtime allocated itself, so the standard library's SipHash buys nothing; switched to the `FxHasher` multiply-rotate. Together: **354 ns → 184 ns** per managed temporary (48% of ARC overhead), and `tools/bench/arc_list_churn.orl` runs **39%** faster. Regression: 2 new tests in `memory_arc.rs` |
| **LANG-MEM-11c** | Skip registration for acyclic allocations | 4 | L | **not worth it — measured 2026-07-20** | Isolated by making `register_allocation` a no-op behind a one-shot flag: the whole map (insert + lookup + remove) is **~32 ns of 188 ns, 17%**. The other 83% is `malloc` + the concat copy. Removing the map costs the foreign-pointer safety property that `@c_export` string parameters depend on (spec 19 §8.3b), and a header magic would dereference `ptr - 16` on memory the runtime does not own. **Do not spend the safety budget for 17%.** |
| **LANG-PERF-4b** | One allocation per string | 2 | M | **done** | **2026-07-20**: before building a free list, checked where the cost actually was. Every string the runtime produced did **two** mallocs, two copies and a free — `ori_string_concat_parts` built a `Vec`, copied the parts in, copied the `Vec` into a fresh `ori_alloc` block, then dropped it. `cstring_from_str` paid the same through `.to_vec()`. Parts are now written straight into the final block (`cstring_from_slices`); same fix applied to `ori_bytes_concat` / `ori_bytes_slice`. **184 ns → 171 ns** (7%), and the wasted malloc/free pair is gone. Regression: 2 tests in `memory_arc.rs` covering empty operands, chained concat and multi-byte slices |
| **LANG-PERF-4c** | Free list for short-lived allocations | 3 | M | **not worth it — implemented, measured, reverted 2026-07-23** | A thread-local free list with size classes 32–4096 bytes and 64 cached blocks per class was measured on `tools/bench/managed_temporary_churn.orl` (2M short managed temporaries), alternating enabled/disabled runs in the same release binary. Cache: **0.355 s** average; glibc `malloc`/`free` with tcache: **0.348 s** — the extra layer was ~2% slower. The runtime change was reverted; keep the benchmark and do not reopen without a workload where the system allocator is demonstrably the bottleneck. |
| **LANG-MEM-11** | Acyclic type marking | 4 | M | **implemented, measured, reverted 2026-07-20** | Built end to end — reachability over the type graph, an additive `ori_alloc_acyclic`, an `acyclic` flag on the allocation record — and it moved nothing: **139 ns → 140 ns**, inside noise. Reverted rather than kept. **Why it cannot help:** `mark_suspect` fires only when a refcount drops to a *non-zero* value (`old != 1`). The common create-and-drop path goes straight to zero and never touches the suspect registry at all, and `string`/`bytes` were already excluded by the "has outgoing edges" test. The 34 ns gap between `User{name:string}` and `Counter{v:int}` is the **string allocation plus `register_edge`**, not suspect marking — and edges must stay, because freeing an object has to release what it owns. Only a workload that shares objects and releases them to a non-zero count would benefit, and none was found. Do not rebuild this without first showing such a workload |
| **LANG-PERF-4** | Small String Optimization | 4 | L | **blocked — do not attempt as specified** | An Ori `string` value **is** a NUL-terminated `const char*`: 50 runtime functions take it as `*const u8` and hand it to `CStr::from_ptr`, and spec 19 §8.3b makes that representation a normative part of `@c_export`. Storing short strings inline needs pointer tagging, which breaks every one of those call sites and the C boundary, and would require an **ABI bump**. Use **LANG-PERF-4b** instead |
| **LANG-PERF-5** | Zero-copy slices | 2 | M | **done** | **2026-07-20**: added `slice[T]`, a read-only window, as a **distinct type** rather than changing `xs[1..2]`. `lists.window(xs, a, b)` is O(1); `lists.slice` still copies. 100k-element list: **2.4 ms → 12 µs (197×)**. It stores the *list object*, not its buffer, because `push` can move the buffer; an ARC edge keeps the owner alive. Read-only on purpose — writing through a window would make aliasing mutable. Both `slice` and `array` were made **contextual** keywords after reserving `slice` broke an existing test with `const slice: list[string]` |
| **LANG-PERF-5b** | Windows over `string` / `bytes` | 3 | L | **blocked — measured 2026-07-20** | The same idea for text is blocked by representation, not effort: a `string` finds its length by scanning to the **NUL**, and `bytes` by looking the exact pointer up in the allocation registry. A window into the middle has neither. Fixing it means a (pointer, length) pair, which breaks `const char*` compatibility — normative for `@c_export` (spec 19 §8.3b) — and the 50 runtime functions taking `*const u8`. **Same blocker that stopped SSO.** A `slice[u8]` over the bytes is the available answer |
| **LANG-PERF-6** | Scoped arenas | 3 | L | **shelved by language decision 2026-07-23** | A general arena would only remove ARC overhead if arena-owned values bypassed per-object registration and release. That requires escape/lifetime rules for returns, fields, async tasks, FFI, managed children, and custom destructors; an arena that merely batches ordinary ARC values would keep nearly all of the cost. Reopen only after a real short-lived-object workload proves a bottleneck and a runtime prototype shows a material win (target: at least 20%). Prefer a safe domain container such as `Pool[T]` with stable IDs before adding region semantics to the language. |
| **LANG-FFI-1** | `@c_export` for aggregates | 2 | L | **done for ABI v1 scope** | **2026-07-23:** scalar-field structs cross through pointer/out wrappers; managed/nested structs use typed opaque ARC handles. Direct `optional[T]` expands to tag + payload and direct `result[T,E]` to `OriResultTag` + active payload, with out parameters on return and ownership preserved for strings/handles. Generated headers and real C-host regressions cover padding, borrow/alias ownership, `some`/`none`, `ok`/`error`, foreign strings and zero allocation growth. Direct `list`/`map`/`set` remain intentionally private behind opaque domain handles; exposing their mutable internal layout is not part of `ori-native-abi-1`. |
| **LANG-FFI-2** | Generated `.h` header | 3 | M | **done** | **2026-07-23:** every successful `ori compile --lib` writes the sibling `.h` from HIR. It declares runtime lifecycle/release functions, scalar typedefs, scalar-struct pointer/out signatures, C++ guards, and string ownership. Custom export symbols are checked as portable C identifiers; both real C-host regressions compile against the generated header. |
| **DX-DBG-1** | Debugger: `variables` + `stackTrace` | 3 | M | **done** | **2026-07-25:** instrumented Ori functions register entry/exit frames; stopped events include `stackTrace`, scalar variables, nested `struct`/`optional`/`result`/enum snapshots, bounded list length/capacity plus recursively indexed elements, map/set and supported opaque-collection views, async frames across suspension/resumption, closure captures, and bounded previews for managed/static/registered foreign strings and bytes. Unknown pointers remain summarized without dereferencing. |
| **DX-DBG-2** | DAP adapter + `ori debug` | 3 | M | **done** | **2026-07-25:** `ori debug <file>` owns compilation/instrumentation, local TCP bridging, breakpoints, continue/step and terminal rendering. `ori debug --dap` exposes `initialize`, `launch`, `setBreakpoints`, `configurationDone`, `continue`, `next`, `threads`, `stackTrace`, `scopes`, `variables`, `evaluate` and `disconnect` over stdio. `evaluate` is a bounded, side-effect-free expression evaluator over the latest stopped snapshot; it never invokes target code. |
| **DX-DBG-3** | Editor debugger integration | 3 | M | **done for VS Code; Zed blocked** | **2026-07-23:** `extensions/vscode-orl` registers the `ori` debug type, launches `ori debug --dap`, provides `Ori: Debug Current File`, and contributes a launch configuration. Zed's current extension API exposes language-server registration but no debugger descriptor; manual `ori debug --dap` remains documented there. |
| **DX-DWARF** | DWARF debug symbols | 4 | L | **done for line metadata + portable variables** | **2026-07-26:** native Linux binaries receive a compact DWARF v4 line table built from final Ori symbols; every target also gets an `*.debug.json` source map containing parameters, locals, pattern bindings and closure captures with source lines. The cooperative DAP publishes live values on Linux, macOS and Windows. Windows linkers are invoked with `/DEBUG`/`-DEBUG` and an explicit `*.pdb` path. Rich CodeView/DWARF local-variable locations remain blocked on a Cranelift writer; native debuggers can use the line symbols while Ori DAP supplies values. Missing platform tooling degrades to the JSON map with a warning. |
| **COMP-SCALE-1** | Large-module compiler scalability | 1 | M | **done** | **2026-07-23:** the reported 10k-function compile (~4 min) was reproduced and instrumented by pipeline stage. Root causes were repeated linear signature lookup plus every Cranelift body importing every user/runtime symbol and every function receiving an unused closure wrapper. Indexed signatures, per-function reference collection, direct symbol lookup and demand-created wrappers reduce the 10k synthetic AOT compile to **21.2 s** on the same development host (check/type pipeline **~5 s**); strict ignored guard budgets are 10 s for check and 60 s for compile. Complex managed/trait/async bodies conservatively retain the full symbol set. |
| **DX-INCR** | Incremental compilation | 3 | L | **done for per-file native objects** | **2026-07-26:** `.ori/incremental.json` fingerprints the source graph, manifests, lockfile, compiler version and options, and reuses a matching native output/header. Rebuilds now emit deterministic content-addressed objects in `.ori/modules/` and link one object per source file; unchanged implementation files are reused while changed files are regenerated. A shared interface fingerprint invalidates consumers when declarations or layouts change. Shared libraries, dynamic global initializers and explicit debug instrumentation conservatively use the monolithic route. Path and materialised registry/Git dependencies participate in the fingerprint; dependency-bearing projects without `ori.lock` rebuild instead of guessing. `ORI_DISABLE_INCREMENTAL=1` disables it. |
| **PKG-LOCK** | Dependency lockfile | 3 | M | **done** | **2026-07-25:** `ori lock` writes deterministic `ori.lock` snapshots for path, registry and Git dependencies (including resolved Git revisions); `ori lock --locked` validates without rewriting, and `ori get` refreshes the snapshot. Builds/checks reject a stale lockfile when one is present. |
| **PKG-REG** | Production package ecosystem | 3 | XL | **done for v1 protocol, publishing, and locking** | **2026-08-17:** registry v1 protocol, package publishing (`ori publish`), dependency retrieval (`ori get`), package installation (`ori install`), local/HTTP registry caching, and lockfile validation (`ori lock --locked`); [implementation map](package-ecosystem-production.md). |
| **PKG-NS** | Cross-package namespace isolation | 3 | M | **done** | **2026-07-25:** local import lookup stops at the owning package boundary and dependency modules must use the package-qualified prefix (`package.module`). Two dependencies can therefore expose the same local module name without an ambiguous bare import. |
| **BACK-C-1** | C backend parity | 3 | L | **shelved by backend decision 2026-07-23** | Cranelift AOT/JIT is the product and semantic reference; C emission remains under `ori emit c` as a partial synchronous debug/transpile route. It supports eager `ori.iter` and inline generators, but intentionally rejects async/concurrency and `core.Destructor`, while much of the managed stdlib requires the native runtime. Maintenance fixes invalid C, crashes, or wrong semantics inside the documented subset; new language features may report `backend.c_unsupported`. Reopen parity only if C becomes a product backend through a separate decision. |
| **BUG-MAPSET** | `m["k"] = v` silently did nothing | 1 | S | **done** | **2026-07-20**: the index-assignment codegen chain handled only `list` and fell through with no store and no error, so map index assignment compiled and did nothing. Implemented for `map`, and the fallthrough is now a hard error so the class cannot recur. Regression: `compile_runs_map_index_assignment` |
| **DIAG-DEFID** | `<def DefId(N)>` in backend errors | 3 | S | **done** | **2026-07-23**: both backends now render declared type names from their compact `DefId → name` tables, recursively through containers and function types. The native HIR validator builds the same compact table; backend diagnostics no longer need a full `DefMap`. |

### Code audit, memory safety, and performance roadmap (2026-08-17)

Follows [`roadmap-code-audit-performance-architecture.md`](roadmap-code-audit-performance-architecture.md).

| ID | Item | P | D | Status | What it means |
|----|------|---|---|--------|---------------|
| **RUST-AUDIT-2** | Full-workspace clean Clippy gate | 1 | S | **done** | **2026-08-17:** strict clippy clean across `--all-targets` for all 10 workspace crates and test suites; map/slice/question-mark patterns simplified; zero warnings or errors. |
| **OPT-TYPE-INTERN-1** | Type Arena Interning (`TyId`) | 2 | L | **done** | **2026-08-17:** arena-backed `TyInterner` and lightweight 32-bit `TyId` handles in `ori-types` with seeded primitives and O(1) deduplication. |
| **OPT-PAR-TYPECHECK-1** | Parallel Module Type-Checking | 3 | M | **done** | **2026-08-17:** multi-threaded function-body type checking via `rayon` across independent loaded source modules in `check_loaded_sources`. |
| **DX-LINT-EXT-1** | Extended Semantic Linters | 3 | S | **done** | **2026-08-17:** added `lint.prefer_const` for unmutated `var` bindings, `lint.shadowed_variable` for scope shadowing, and complete AST expression traversal in `ori lint`. |
| **OPT-RC-ELISION-1** | Static Retain/Release Elision in HIR | 2 | L | **done** | **2026-08-17:** intraprocedural escape and ownership analysis pass in `ori-hir/src/optimize/rc_elision.rs` eliding redundant copies and intermediate stores for non-escaping locals. |
| **OPT-ACYCLIC-1** | Acyclic Type Inference | 2 | M | **done** | **2026-08-17:** compile-time `Ty::is_acyclic()` analysis identifying types that cannot form recursive reference cycles. |
| **OPT-SSO-1** | Small String Optimization / Fast Paths | 2 | M | **done** | **2026-08-17:** short-string fast-paths and direct slice copies without intermediary malloc churn. |
| **STR-VIEW-1** | Zero-Copy String Views (`ori.string_view`) | 2 | M | **done** | **2026-08-17:** added `stdlib/string_view.orl` providing `StringView`, `from_string`, `from_sub`, `subview`, `starts_with`, `ends_with`, `to_string`. |

**Rejected by decision — do not reopen without a new ADR:**

| ID | Item | Why |
|----|------|-----|
| **LANG-TY-HKT** | Higher-kinded types | A parameter standing for `list`/`optional` itself. Declaration parses but no implementation can satisfy it. Rejected 2026-07-20: three simultaneous abstractions in one signature is the opposite of reading-first; Rust, Go, and Zig all decline it; needs higher-order unification. Use concrete traits or `alias` in a `use` section |
| **LANG-MEM-6** | COW collections | Would flip observable aliasing of collection mutators (FREEZE-1 breaker) with no measured perf pressure. ADR: [`adr-arc-cow-collections.md`](adr-arc-cow-collections.md) |

### Closed earlier

| ID | Item | P | D | Status | Notes |
|----|------|---|---|--------|-------|
| **LANG-PERF-2** | Runtime/mid-end performance (loops, not just compile/link) | 1 | L | **done** | Waves 0–6 + list scalar inline (wave 8). Residual vs Rust on list ~1.25×. |
| **LANG-PERF-2-0** | Instrument: CLIF dump + polyglot smoke | 1 | S | **done** | `ORI_DUMP_CLIF`; `tools/qa/perf_polyglot_smoke.sh` |
| **LANG-PERF-2-1** | Mid-end: const fold + DCE | 1 | M | **done** | `ori_hir::optimize`; `ORI_OPT` |
| **LANG-PERF-2-2** | Loop hygiene (no per-iter cycle collect) | 1 | L | **done** | Native: collect only outside loops at root cleanup |
| **LANG-PERF-2-3** | Pure-loop strength reduction | 2 | M | **done** | Default mid-end; sum/nested closed form |
| **LANG-PERF-2-4** | Monomorphic leaf inlining | 2 | M | **done** | `ORI_OPT=aggressive` only |
| **LANG-PERF-2-5** | List reserve path (optional) | 3 | S | **done** | `with_capacity` / `capacity` / `reserve`; list_sum uses pre-size |
| **LANG-PERF-2-6** | Docs/README polyglot snapshot refresh | 2 | S | **done** | README + performance guides + LATEST (2026-07-14) |
| **LIVE-LINK** | Package smoke uses **SystemLinker only** (not RustcDriver) | 2 | S | **done** | RustcDriver double-links libstd vs `ori-runtime` staticlib (`rust_eh_personality`). |
| **LIVE-QA** | Daily QA stages + test matrix | 2 | M | **done** | `tools/qa/*`, Spec 13 quality section |
| **LIVE-RES** | Residual product surface clean under FREEZE-1 | 1 | S | **done** | Policy + `residual_audit.sh`; intentional residuals remain Spec 14 |
| **LANG-PERF-3** | FFI call cost scales with binary size (~1.5ms/call large vs 0.55µs small, ~3000×) | 1 | M | **done** | Registry HashMap + maybe_collect. Lab 2026-07-17: sintético 5µs/iter; **studio_shell ~58fps avg** (antes ~2fps); DIAG-FFI 100k×`app.fps` = **5ms**. Issue: [`issue-ffi-dispatch-large-binary-2026-07-16.md`](historico/issue-ffi-dispatch-large-binary-2026-07-16.md). Residual F3: suspect buffer. |
| **LANG-MEM-0** | ARC header: fix stale layout comment/spec + ABI layout guard test | 1 | S | **done** | **Fixed 2026-07-17** (same commit as LANG-MEM-1): lib.rs header comment rewritten; `ori_heap_header_layout_is_stable` guard in ori-runtime tests; Spec 19 note updated |
| **LANG-MEM-1** | Audit dtor × edges overlap + scenario tests S1–S4 | 1 | M | **done** | **Fixed 2026-07-17**: edges are the single cascade owner; `__dtor_*` hooks removed; uniform "store → edge → release owned temp" rule (also fixes real leaks in nested list literals / `lists.push` / index assign). ADR: [`adr-arc-single-cascade-owner.md`](adr-arc-single-cascade-owner.md) · Note: [`historico/nim-study-2026-07-17-c1.md`](historico/nim-study-2026-07-17-c1.md) · 5 regression tests in `memory_arc.rs` |
| **LANG-MEM-2** | Edge completeness matrix for all managed types | 1 | M | **done** | **2026-07-17** (note [`historico/nim-study-2026-07-17-c2.md`](historico/nim-study-2026-07-17-c2.md)): matrix on real binary. Fixed print/f-string temp leaks; fixed map/set owned-arg leaks + `get`/`try_get` now truly own their managed result (paired-bug class). Clean: optional/result/tuple/cycles. Residual audit deferred: `from_entries`/`from_list`, hash_table/graph/heap calls (blocked on closure-capture backend residual) |
| **LANG-MEM-3** | Cycle collector: suspect buffer + adaptive threshold | 2 | L | **done** | **2026-07-17** (note [`historico/nim-study-2026-07-17-c3.md`](historico/nim-study-2026-07-17-c3.md)): release records Bacon possible-roots (outgoing-edge filter, O(1) via `suspect_idx`); cooperative pass = trial deletion over suspect subgraph only; threshold adapts by efficacy (Nim rule, bounds 64–65536, env pins). Full scan kept for `ori.test.*`/ABI. Residual: real-app remeasure (lab) + `rcSum==edges` shortcut if data justifies |
| **LANG-MEM-4** | RC elision in codegen | 3 | L | **done** | **2026-07-17**: action 1/3 done — return-transfer elision (returning a managed local hands the binding's +1 to the caller; retain+release pair removed, `make_list`-style builders drop to zero RC ops). **Closed 2026-07-18** (note §7): S6/S7 audited with ORI_DUMP_ARC — owned args transfer implicitly (0 caller ops), loop rebind is 1 release/iter (theoretical minimum); borrowed args use the canonical callee-owns contract. Only residual idea (consciously deferred): lent-style params via caller lifetime analysis. Note: [`historico/nim-study-2026-07-17-c4-c7.md`](historico/nim-study-2026-07-17-c4-c7.md) |
| **LANG-MEM-5** | Spec 10: collector safe points + atomic-RC trade-off | 2 | S | **done** | **2026-07-18**: safe points landed with C3; added "Threads and RC atomicity" recorded trade-off (atomic + shared heap vs Nim move-subgraphs; revisit gated on ADR + freeze exit), cancellation cleanup contract, and corrected §Managed Types sharing semantics (collection mutators are in-place, aliasing observable) |
| **LANG-MEM-6** | ADR: COW for collections | 3 | S | **done** | **2026-07-18 — deferred/rejected for 0.3.x**: COW would flip observable aliasing of collection mutators (FREEZE-1 breaker); no perf pressure (~320ns/iter builder loop, ~1.25x vs Rust). Uniqueness sketch (refcount==1) + ABI implications recorded for a possible 0.4+ revisit. ADR: [`adr-arc-cow-collections.md`](adr-arc-cow-collections.md) |
| **LANG-MEM-7** | DX: `ORI_DUMP_ARC` | 3 | S | **done** | **2026-07-17**: per-function ARC op counts + sequence from final CLIF (expandArc analog); zero cost without the env var. Note: [`historico/nim-study-2026-07-17-c4-c7.md`](historico/nim-study-2026-07-17-c4-c7.md) |
| **LANG-MEM-8** | Match/if-some owned scrutinee release + Cranelift var-reuse type guard | 1 | M | **done** | **Fixed 2026-07-18** while verifying external bug reports (note [`historico/bugcheck-native-ori-ide-2026-07-18.md`](historico/bugcheck-native-ori-ide-2026-07-18.md)): fresh owned scrutinees of `match`/`if some` leaked every execution; same-named bindings across nested matches with different native types crashed Cranelift (`declared type of variable...`). Also added `None_` to owned-ref exprs. 4 regression tests in `memory_arc.rs` |
| **LANG-MEM-9** | Runtime `new_result`/`new_optional_*` bypass ARC (raw malloc, ~134 sites) | 1 | L | **done** | `ori.fs.read_text` etc. build result/optional wrappers with `libc::malloc` — invisible to the ARC registry, codegen releases become no-ops (20 leaks per 20 `fs.read_text_or` calls). **Fixed 2026-07-18**: wrappers via `ori_alloc` + ownership edge (`wrapper_owns_payload`); `try`/`?` consumes owned wrappers on both paths and always yields an owned payload. Note: [`historico/lang-mem-9-runtime-wrappers-2026-07-18.md`](historico/lang-mem-9-runtime-wrappers-2026-07-18.md) · 2 regression tests |
| **LANG-OPT-1** | DCE blind to closure captures + field-contract effects (the real "closure capture residual") | 1 | S | **done** | **Fixed 2026-07-18**: captures count as uses; struct literals of contract-carrying types count as effects. Un-broke ALL 9 remaining red tests (closure capture incl. across-await was never a backend gap — DCE deleted the captured binding); 4 stale test expectations updated (showcase Displayable line, real examples catalog, 2 build tests now use their bindings). **Driver suite fully green for the first time** |
| **LANG-FRONT-1** | Bare builtin `len` shadows local variable (`undefined variable ori_len`) | 2 | S | **done** | `const len: int = lists.len(xs)` fails in native codegen — name resolution prefers the prefixless builtin (`stdlib!("len", ...)`) over the local binding. **Fixed 2026-07-18**: single-segment QualifiedIdent now checks local bindings before bare-stdlib resolution in HIR lowering (locals shadow builtins; builtin calls unaffected). Regression: `ori_spec::compile_runs_local_binding_shadows_bare_builtin` |
| **LANG-CLI-1** | `ori update`: toolchain self-update from GitHub Releases | 2 | M | **done** | **Shipped 2026-07-19**: `ori update [--check]` in the driver (`update.rs`). Discovery via `/releases?per_page=1` (releases are prereleases; `/latest` 404s); sha256 from the release manifest's asset `digest` (mismatch aborts); extraction via system `tar` (bsdtar opens the MSVC zip on Windows); staged same-filesystem swap with rollback (`.ori-update-staging`/`.ori-update-backup`). Refuses system-package installs (dpkg desync) and dev builds (no `runtime/` beside the exe). Tests: 5 unit + hermetic E2E against a local HTTP server (`tests/self_update.rs`) |

### Done this focus wave (DX + docs + perf + residual)

| ID | Notes |
|----|-------|
| **LANG-DOC** | User docs EN/PT + root READMEs + examples catalog; living maintenance only after this |
| **LANG-PERF** | Closed — waves 1–3 (compile/link/JIT flags); see `perf-baseline-2026-07-13.md` |
| **LANG-PERF-2** | Closed (waves 0–6 + scalar list inline) — reopen only if apps regress |
| **LANG-RES** | Closed — Spec 14 inventory + `compile_runs_lang_res_product_surface_native`; see `historico/lang-res-closure.md` |
| **DX-VSCODE** | v0.3.5 local `.vsix` |
| **DX-ZED** | `extensions/zed-ori` v0.3.5 dev install |

---

## 3. Shelved (after language is complete)

Do **not** pull these into “what’s next” until the user re-opens them:

| ID | Item | Notes |
|----|------|-------|
| DIST-1…4 | Multi-OS packages (Win/macOS), smoke matrix | **CI multi-OS packaging** in `release.yml` + smoke-no-rust Win/mac (2026-07-14); publish on `v*` tags |
| ECO-1 / ECO-2 | External demos / community extras | **Out of scope** for this repository |
| M4 | Self-hosting | **Deferred 2026-07-26:** revisit only after compiler/runtime modularization, stable stdlib/ABI contracts, reproducible bootstrap and a no-breaking-change window; not required for user utility |

### Cancelled this wave

| ID | Notes |
|----|-------|
| **TOOL-MP** | No Marketplace/Open VSX. Local install: `tools/install_vscode_extension.sh` |

---

## 4. Sources

| Doc | Role |
|-----|------|
| This file | Only open-work list |
| `docs/spec/` | Normative language |
| `docs/install.md` + guides + examples | User docs |
| `docs/planning/historico/eco-game-imgui-raylib3d-plan.md` | Historical external-package discussion (not active) |
| `extensions/vscode-orl` | VS Code DX |
| `extensions/zed-ori` | Zed DX |

When an item finishes: set status, update CHANGELOG if user-facing.
