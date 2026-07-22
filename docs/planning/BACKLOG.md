# Ori — single implementation backlog

> **This file is the only active “what remains to implement” list.**  
> Surface baseline: **S3 `0.3.0`** + inference B **`0.3.1`** + package **`0.3.4`**.  
> Last consolidated: **2026-07-20** — §2 reopened with the real remaining work
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

**Not in monorepo product tree:** `ori-game` / `ori-imgui` remain **external packages** (sibling repos). Revival work follows `docs/planning/eco-game-imgui-raylib3d-plan.md` — do **not** re-vendor into `ori-lang` unless a new explicit decision says so.  
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
| DONE-FREEZE-1 / ABI-1 | Freeze window open; ABI-1 in force; readiness checklist finalized |
| DONE-DIST-LINUX-DEB | Linux `.tar.gz` + `.deb` via `package_native_release` / `package_deb`; CI release assets |
| DONE-LANG-DOC | User docs + examples aligned to S3 / current stdlib / editors local |
| DONE-LANG-PERF | AOT/JIT, stage release, mold/lld PATH, microbench + ARC bench; living JIT lower only |
| DONE-LANG-RES | Native residual inventory Spec 14; product surface gate test; reopen only on concrete blocker |
| CANC-GAME / CANC-IMGUI | **Cancelled as monorepo product** — external plan: `eco-game-imgui-raylib3d-plan.md` |
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

| ID | Item | P | D | Status | What it means |
|----|------|---|---|--------|---------------|
| **LANG-TY-1** | Fixed-size arrays | 1 | M | **done** | **2026-07-20**: `array[T, size: N]`, stored inline (stack slot for locals, inline bytes inside structs). Surface chosen to match const generics rather than Rust's `[int; 4]`, which would have introduced `;`. Length may be a `const` parameter, so `InlineString[const cap: int]` works. Constant indices are bounds-checked at compile time. Managed element types rejected (`type.array_element_not_inline`) because inline storage has no ARC. Native backend only. Found and fixed a pre-existing bug on the way — see **BUG-MAPSET** |
| **LANG-TY-2** | Applicable generic traits | 2 | M | **done** | **2026-07-20**: all four layers landed — `ApplyUseSection.trait_args`, the parser accepting `[` after a trait name, `TraitSig.type_params`, `ImplSig.trait_args` — plus positional binding in the checker (call sites *and* impl validation) and in HIR lowering, which resolved its own return types and would otherwise hand codegen an unresolved `Item`. `substitute_trait_self` moved from `check.rs` to `ty.rs` since it now has callers in two crates. New diagnostics: `impl.trait_args_missing`, `impl.trait_arg_count_mismatch` |
| **LANG-TY-8** | Receiver-less trait methods (associated functions) | 3 | M | **done** | **2026-07-22**: the rule is now *explicit `self` = instance method; no `self` = associated function*, called `Type.method()`. Implicit self (a `greet()` body reading `self.name`) was removed — it was undocumented leniency contradicting every spec example, and 6 of our own tests relied on it (all updated to explicit `self`). `core.Default` is real: `default() -> Self`, validated by the impl machinery (`impl.missing_method` / `impl.wrong_signature` fire naturally). Trait-section associated fns alias to `ns.Type.method`; visibility of type-scoped paths follows the module. New diagnostic: `type.assoc_fn_instance_call`. Statics are excluded from `any[Trait]` dispatch. Remaining: `T.default()` on a generic bound (needs monomorphized static dispatch) |
| **LANG-TY-3** | `Cloneable` with real methods | 2 | M | **done** | **2026-07-20**: the fix was one place. `substitute_trait_self` already existed and was applied on the generic-parameter path, but not on the concrete-type path, so a trait method returning `Self` reported the *trait* as its return type. `Cloneable` now carries `clone(self) -> Self`, and user traits returning or taking `Self` work as well. `Default` stays a marker — `default() -> Self` needs a **receiver-less** trait method, which is a different gap |
| **LANG-TY-4** | `comptime` | 3 | L | todo | Run code at compile time (build tables, validate constants) without macros |
| **LANG-TY-5** | Generators: `iter` + `suspend` (B1, inline) | 3 | M | **done** | **2026-07-22**: shipped as Nim-style *inline* iterators after studying Nim's two-tier design (inline = AST splice, closure = state machine) — the common case needs no state machine, which cut the estimate from L to M. `iter f(...) -> T` + `suspend v`, consumed only by `for`; body inlined at the loop site via AST→AST transform in `ori-hir` (one-shot loop + done-flag cascade, no labeled breaks, both backends free). Bench: eager `iter.map`+`filter` chain 330 ms vs generator 24 ms (200k×20, **13×**). Keyword is `suspend` (Icon precedent) — says what happens; `yield` is ambiguous. 11 dedicated diagnostics, specs 02/03/06/07/13 + tour, 4 e2e tests. Decision record: `ori.iter` stays **eager** (like Nim's `sequtils`); no lazy module will be added. Remaining tier: **LANG-TY-5b** |
| **LANG-TY-5b** | First-class iterators (B2) | 4 | L | todo | Store/pass an iterator as a value (state-machine compilation, reusing the async frame plan). Also lifts the B1 limits: methods, cross-module, generics, recursion. Only worth it when a real use case appears — the `for`-direct case is fully covered by LANG-TY-5 |
| **LANG-TY-6** | Explicit `move` | 3 | M | todo | Transfer ownership without a refcount bump |
| **LANG-TY-7** | Custom destructors | 3 | M | todo | Run user code when a value dies (close a socket, free an external resource) |
| **LANG-MEM-10** | Static retain/release elision | 3 | L | **re-scoped 2026-07-20** | **Measured before implementing, and the premise did not hold.** `ORI_DUMP_ARC` on a managed-temporary hot loop shows `release=2, retain=0` — there is no retain/release *pair* to elide. LANG-MEM-4 had already driven retains to the minimum. The real cost is the **allocation registry**: every managed object takes the ARC mutex and touches a `HashMap` on alloc and on free. Dropping one string took **six** mutex acquisitions. Consolidating `ori_arc_release` into one critical section landed **19% off** a 2M-iteration loop (360 ns → 290 ns per managed temporary). What remains of the original idea is low-value; the follow-up with headroom is **LANG-MEM-11** |
| **LANG-MEM-11b** | Cheaper ARC bookkeeping | 1 | M | **done** | **2026-07-20**: two ABI-neutral changes. (1) `ori_arc_release` consolidated from up to six mutex acquisitions into one. (2) The ARC maps are keyed by pointers the runtime allocated itself, so the standard library's SipHash buys nothing; switched to the `FxHasher` multiply-rotate. Together: **354 ns → 184 ns** per managed temporary (48% of ARC overhead), and `tools/bench/arc_list_churn.orl` runs **39%** faster. Regression: 2 new tests in `memory_arc.rs` |
| **LANG-MEM-11c** | Skip registration for acyclic allocations | 4 | L | **not worth it — measured 2026-07-20** | Isolated by making `register_allocation` a no-op behind a one-shot flag: the whole map (insert + lookup + remove) is **~32 ns of 188 ns, 17%**. The other 83% is `malloc` + the concat copy. Removing the map costs the foreign-pointer safety property that `@c_export` string parameters depend on (spec 19 §8.3b), and a header magic would dereference `ptr - 16` on memory the runtime does not own. **Do not spend the safety budget for 17%.** |
| **LANG-PERF-4b** | One allocation per string | 2 | M | **done** | **2026-07-20**: before building a free list, checked where the cost actually was. Every string the runtime produced did **two** mallocs, two copies and a free — `ori_string_concat_parts` built a `Vec`, copied the parts in, copied the `Vec` into a fresh `ori_alloc` block, then dropped it. `cstring_from_str` paid the same through `.to_vec()`. Parts are now written straight into the final block (`cstring_from_slices`); same fix applied to `ori_bytes_concat` / `ori_bytes_slice`. **184 ns → 171 ns** (7%), and the wasted malloc/free pair is gone. Regression: 2 tests in `memory_arc.rs` covering empty operands, chained concat and multi-byte slices |
| **LANG-PERF-4c** | Free list for short-lived allocations | 3 | M | todo | What is left per temporary is one `malloc`/`free` pair plus the copy. A size-classed free list inside `ori_alloc` would reuse recently freed blocks, keeping the payload representation and the registry unchanged — no ABI or safety impact. Measure first: glibc's tcache is already a fast path, so the headroom may be small |
| **LANG-MEM-11** | Acyclic type marking | 4 | M | **implemented, measured, reverted 2026-07-20** | Built end to end — reachability over the type graph, an additive `ori_alloc_acyclic`, an `acyclic` flag on the allocation record — and it moved nothing: **139 ns → 140 ns**, inside noise. Reverted rather than kept. **Why it cannot help:** `mark_suspect` fires only when a refcount drops to a *non-zero* value (`old != 1`). The common create-and-drop path goes straight to zero and never touches the suspect registry at all, and `string`/`bytes` were already excluded by the "has outgoing edges" test. The 34 ns gap between `User{name:string}` and `Counter{v:int}` is the **string allocation plus `register_edge`**, not suspect marking — and edges must stay, because freeing an object has to release what it owns. Only a workload that shares objects and releases them to a non-zero count would benefit, and none was found. Do not rebuild this without first showing such a workload |
| **LANG-PERF-4** | Small String Optimization | 4 | L | **blocked — do not attempt as specified** | An Ori `string` value **is** a NUL-terminated `const char*`: 50 runtime functions take it as `*const u8` and hand it to `CStr::from_ptr`, and spec 19 §8.3b makes that representation a normative part of `@c_export`. Storing short strings inline needs pointer tagging, which breaks every one of those call sites and the C boundary, and would require an **ABI bump**. Use **LANG-PERF-4b** instead |
| **LANG-PERF-5** | Zero-copy slices | 2 | M | **done** | **2026-07-20**: added `slice[T]`, a read-only window, as a **distinct type** rather than changing `xs[1..2]`. `lists.window(xs, a, b)` is O(1); `lists.slice` still copies. 100k-element list: **2.4 ms → 12 µs (197×)**. It stores the *list object*, not its buffer, because `push` can move the buffer; an ARC edge keeps the owner alive. Read-only on purpose — writing through a window would make aliasing mutable. Both `slice` and `array` were made **contextual** keywords after reserving `slice` broke an existing test with `const slice: list[string]` |
| **LANG-PERF-5b** | Windows over `string` / `bytes` | 3 | L | **blocked — measured 2026-07-20** | The same idea for text is blocked by representation, not effort: a `string` finds its length by scanning to the **NUL**, and `bytes` by looking the exact pointer up in the allocation registry. A window into the middle has neither. Fixing it means a (pointer, length) pair, which breaks `const char*` compatibility — normative for `@c_export` (spec 19 §8.3b) — and the 50 runtime functions taking `*const u8`. **Same blocker that stopped SSO.** A `slice[u8]` over the bytes is the available answer |
| **LANG-PERF-6** | Scoped arenas | 3 | L | todo | Allocate a whole phase together and release it in one step (frame loops) |
| **LANG-FFI-1** | `@c_export` for aggregates | 2 | L | todo | Structs, `list`, `map`, `optional`, `result` across the C boundary. Needs a stable C layout added to ABI-1. Scalars and `string` already cross (2026-07-20) |
| **LANG-FFI-2** | Generated `.h` header | 3 | M | todo | Emit the C header from the exported functions instead of hand-writing it |
| **DX-DBG-1** | Debugger: `variables` + `stackTrace` | 3 | M | todo | **Corrects an earlier wrong note here.** Ori *does* have a working line debugger: `ORI_DEBUG_INSTRUMENT=1` + `ORI_DEBUG_SOURCE` at compile time emit `ori_debug_line` probes, and `ORI_DEBUG_PORT` at run time starts the cooperative agent in `ori-runtime/src/debug_agent.rs`. Verified end to end 2026-07-20: breakpoint by file:line, `step`, `continue`, `terminate` all work. What is missing is **seeing your data**: the agent has no `variables` and no `stackTrace`, so you stop but cannot inspect. Needs the compiler to emit a per-function descriptor (name, type, frame offset) and the probe to pass the frame pointer. **Deliberately sequenced after `yield` / `move` / custom destructors**, each of which changes what a frame looks like |
| **DX-DBG-2** | DAP adapter + `ori debug` | 3 | M | todo | No adapter ships in this repo (the `ori-dap` the agent's docs mention is not here), and enabling the debugger needs two env vars at compile time plus a port at run time. Low value until DX-DBG-1 lands |
| **DX-DWARF** | DWARF debug symbols | 4 | L | todo | Native GDB/LLDB, plus perf/valgrind understanding Ori frames. Independent of the cooperative agent above and much larger; worth it only if external tooling becomes a goal |
| **DX-INCR** | Incremental compilation | 3 | L | todo | Rebuild only what changed; every build is currently full |
| **PKG-LOCK** | Dependency lockfile | 3 | M | todo | Pin exact versions for reproducible builds |
| **PKG-REG** | Official registry | 4 | L | shelved | Only `ORI_REGISTRY` pointing at your own endpoint today |
| **PKG-NS** | Cross-package namespace isolation | 3 | M | todo | Keep two packages from colliding on module paths |
| **BACK-C-1** | C backend parity | 3 | L | todo | Rejects async, concurrency, and `ori.iter` with `backend.c_unsupported`. It is a debug aid, not a semantic reference — closing this is optional |
| **BUG-MAPSET** | `m["k"] = v` silently did nothing | 1 | S | **done** | **2026-07-20**: the index-assignment codegen chain handled only `list` and fell through with no store and no error, so map index assignment compiled and did nothing. Implemented for `map`, and the fallthrough is now a hard error so the class cannot recur. Regression: `compile_runs_map_index_assignment` |
| **DIAG-DEFID** | `<def DefId(N)>` in backend errors | 3 | S | todo | Codegen messages still print internal ids; the checker stopped doing so on 2026-07-20. Backends have no `DefMap` in reach |

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
| **LIVE-QA** | Daily QA stages + test matrix + skill `ori-lang-qa` | 2 | M | **done** | `tools/qa/*`, `.grok/skills/ori-lang-qa`, agents, Spec 13 quality section |
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
| **DX-VSCODE** | v0.3.2 local `.vsix` |
| **DX-ZED** | `extensions/zed-ori` dev install |

---

## 3. Shelved (after language is complete)

Do **not** pull these into “what’s next” until the user re-opens them:

| ID | Item | Notes |
|----|------|-------|
| DIST-1…4 | Multi-OS packages (Win/macOS), smoke matrix | **CI multi-OS packaging** in `release.yml` + smoke-no-rust Win/mac (2026-07-14); publish on `v*` tags |
| ECO-1 / ECO-2 | External demos / community extras | Covered by ECO-* plan rows below |
| **ECO-GAME** | Adapt **ori-game** to S3 + raylib 2D + smoke | Plan §3 |
| **ECO-GAME-O** | Camada Ori: tween, scene, assets, save JSON | **Done** 2026-07-13 — plan §9 |
| **ECO-IMGUI** | Adapt **ori-imgui** (Dear ImGui GLFW+GL3) | **Done** MVP 2026-07-13 |
| **ECO-RL3D** | Raylib 3D **draw** + R3 raycast | **Done** 2026-07-13 — plan §5 (R0–R3 pick) |
| **ECO-RAYGUI** | Translate **raygui** → `ori-raygui` | **Done** 2026-07-13 — plan §6 |
| **ECO-BOX2D** | Translate **Box2D** → `ori-box2d` | **Done** MVP 2026-07-13 — plan §7 (milli-unit int FFI) |
| **ECO-JOLT** | Translate **Jolt** → `ori-jolt` | **Done** MVP 2026-07-13 — plan §8 (`ori_jolt_*`, stub/real) |
| **ECO-RRES** | Translate **rres** → `ori-rres` | **Done** MVP 2026-07-13 — ORPK + CRC32 |
| **ECO-SQLITE** | Translate **SQLite** → `ori-sqlite` | **Done** MVP 2026-07-13 — amalgamation + shim |
| **ECO-FUTURE** | Spine, net, compressão avançada, … | Plan §17 only — **not** current scope |
| M4 | Self-hosting | Last language discussion |

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
| `docs/planning/eco-game-imgui-raylib3d-plan.md` | External ori-game / imgui / raylib 3D / raygui |
| `extensions/vscode-orl` | VS Code DX |
| `extensions/zed-ori` | Zed DX |

When an item finishes: set status, update CHANGELOG if user-facing.
