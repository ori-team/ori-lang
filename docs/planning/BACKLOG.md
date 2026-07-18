# Ori — single implementation backlog

> **This file is the only active “what remains to implement” list.**  
> Surface baseline: **S3 `0.3.0`** + inference B **`0.3.1`** + package **`0.3.4`**.  
> Last consolidated: **2026-07-14** (LANG-PERF-2 closed; living QA scripts/skill landed).

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
| **LANG-FRONT-1** | Bare builtin `len` shadows local variable (`undefined variable ori_len`) | 2 | S | todo | `const len: int = lists.len(xs)` fails in native codegen — name resolution prefers the prefixless builtin (`stdlib!("len", ...)`) over the local binding. Frontend fix (ori-types/ori-hir). Note: bugcheck 2026-07-18 §5.2 |

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
