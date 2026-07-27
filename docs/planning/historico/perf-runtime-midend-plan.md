# Plan: runtime / mid-end performance (LANG-PERF-2)

> **Status:** proposed implementation plan (2026-07-13)  
> **Audience:** maintainers / implementers  
> **Surface:** FREEZE-1 on **0.3.x** — no user-facing syntax break without bump + freeze note  
> **Evidence:** polyglot microbench (`tools/bench/polyglot/`, `docs/guides/performance.md`)  
> **Supersedes for *runtime* work:** living residual of `DONE-LANG-PERF` (waves 1–3 were compile/link/JIT flags only)  
> **Out of scope for this plan:** LLVM ORC / alternate JIT backends — **discussion after this plan** (see §12)

---

## 1. Problem statement

Ori AOT already **beats CPython and CRuby** on microkernels and is **near Rust/C/Go on list push+sum** (~1.2–1.6×). The remaining cliff is **tight integer loops** (`fib_iter`, honest `sum`/`nested`): Ori is roughly **30–75×** behind mature AOT (C/Rust/Go/Nim) and can lose to Node on simple arithmetic.

Root cause (product reading, not blame):

1. **Almost no mid-end** between typed HIR and Cranelift lower (`native_backend.rs` is a large direct lower).
2. **Per-iteration overhead** likely includes checks / generic patterns that C/Rust eliminate.
3. Cranelift with `opt_level=speed` cannot invent closed forms or CSE that never appear in the IR it receives.
4. LANG-PERF waves 1–3 improved **compile/link/JIT startup**, not **generated loop quality**.

This plan closes that gap in **incremental, measurable PRs** without changing S3 surface or the public ABI (`ori-native-abi-1`).

---

## 2. Goals

| # | Goal | Success metric |
|---|------|----------------|
| G1 | Make tight loops generate competitive machine code | `fib_iter` Ori/Go ≤ **10×** (today ~28×); stretch ≤ **5×** |
| G2 | Keep list path strong | `list_sum` Ori/Rust ≤ **2×** (today ~1.6×) — **no regression** |
| G3 | Honest arithmetic competitiveness | `nested` Ori/Go ≤ **5×** (today ~28×) |
| G4 | Measurable, automatable | `tools/bench/polyglot` + optional asm dump gate in CI optional/nightly |
| G5 | Safe under FREEZE-1 | No breaking surface; opts are **semantics-preserving** |

### Non-goals (this plan)

- Replacing Cranelift with LLVM (or ORC) as the product backend — **deferred to §12 conversation**.
- Changing ARC to a tracing GC.
- Self-host (M4).
- Marketplace / multi-OS distribution push.
- “Win closed-form `sum` against LLVM” as a vanity metric (unless mid-end strength-reduces it intentionally and tests document it).

---

## 3. Baseline (do not regress)

From polyglot 2026-07-13 (i7-3632QM, median of 3, AOT Ori 0.3.4):

| Kernel | Ori | Go | Rust | C | Py | Node |
|--------|-----|-----|------|---|-----|------|
| `fib_iter` | 0.65 s | 0.023 s | 0.009 s | 0.013 s | 11.2 s | 1.60 s |
| `list_sum` | 0.017 s | 0.014 s | 0.010 s | 0.011 s | 1.00 s | 0.14 s |
| `nested` | 0.12 s | 0.004 s | 0.004 s | 0.002 s | 1.04 s | 0.08 s |
| `sum_loop` | 0.33 s | 0.017 s | ~0\* | ~0\* | 3.21 s | 0.10 s |

\* Rust/C may strength-reduce `sum_loop` — **not** the primary acceptance kernel.

**Primary acceptance kernels:** `fib_iter`, `list_sum`, `nested`.  
**Secondary:** `sum_loop` (after strength reduction lands, document expected closed form or leave as loop).

Harness: `SAMPLES=5 ./tools/bench/polyglot/run_polyglot_bench.sh`  
Update `docs/guides/performance.md` + README snapshot only when medians move ≥ ~15% or goals are hit.

---

## 4. Architecture

### 4.1 Pipeline today

```text
.orl → lexer → parser → resolve → typecheck → HIR
  → monomorph (as needed)
  → native_backend (HIR → Cranelift IR directly)
  → object → link  |  JITModule
```

### 4.2 Pipeline target (this plan)

```text
.orl → … → HIR (typed)
  → monomorph
  → **mid-end** (HIR → HIR, semantics-preserving)     [NEW]
  → native_backend (lower + local peepholes)
  → Cranelift (opt_level=speed AOT / none JIT)
  → object → link | JIT
```

**Design choice:** mid-end operates on **HIR** (already typed, already monomorphized for codegen), not a brand-new SSA IR in wave 1.

Rationale:

- HIR already exists (`ori-hir`); adding a second full IR (CFG/SSA) is a multi-month project.
- Many high-impact opts (const fold, DCE, simple CSE, inline of monomorphic leafs) are tractable on HIR.
- Later (wave optional / post-plan): extract **CFG-SSA** only if HIR mid-end plateaus.

### 4.3 Placement in the workspace

| Piece | Location | Notes |
|-------|----------|--------|
| Mid-end passes | New module `ori-hir/src/optimize/` **or** crate `ori-midend` | Prefer **`ori-hir` module first** (Rule of Three: extract crate only if it grows past ~2–3 kLOC / multiple consumers) |
| Pass driver | `ori-hir::optimize::run_pipeline(hir, OptLevel)` | Called from `ori-driver` / codegen entry before native lower |
| Flags | `OptLevel::{None, Default, Aggressive}` | AOT product = Default; JIT can use None or Default (measure) |
| Golden tests | `ori-hir` unit tests + `ori-driver` compile_runs | IR dumps as strings or structural asserts |
| Asm / IR dump (dev) | `ori compile --emit-clif` or env `ORI_DUMP_CLIF=1` | PR0 — diagnostic only |

### 4.4 Opt levels

| Level | When | Passes |
|-------|------|--------|
| `None` | tests that need deterministic raw lower; optional JIT | no mid-end |
| `Default` | **product AOT** (`ori compile`, `ori test` AOT) | const + DCE + propagate + simple CSE + safe loop IV cleanup |
| `Aggressive` | opt-in env `ORI_OPT=aggressive` | + inline leafs + strength reduction + LICM-lite |

Default must be **safe** (no change to observable I/O, panic, ARC identity where specified).

---

## 5. Work packages (waves)

### Wave 0 — Instrument & baseline (1 PR)

**Why first:** without asm/IR visibility, later PRs are guesswork.

| Deliverable | Detail |
|-------------|--------|
| CLIF dump | After lower, optional write of Cranelift IR text for each function |
| Ops-per-iteration checklist | Doc section in this plan or `docs/planning/perf-loop-checklist.md` filled for `fib_iter` |
| Polyglot “gate script” | `tools/qa/perf_polyglot_smoke.sh` — runs fib+list only, 1 sample, fails if Ori binary missing |
| Capture baseline numbers | Store in `docs/planning/perf-baseline-2026-07-13.md` Wave 5 table (copy from LATEST) |

**Acceptance:**

- [ ] `ORI_DUMP_CLIF=1 ori compile tools/bench/polyglot/ori/fib_iter.orl` produces readable IR
- [ ] Documented count of Cranelift ops in the hot loop of `fib_iter` (manual once)
- [ ] `tools/qa/perf_polyglot_smoke.sh` exits 0 on a machine with `ori` + gcc/go optional

**Risk:** low. No runtime change.

---

### Wave 1 — Mid-end skeleton + safe scalar opts (1–2 PRs)

| Pass | Behavior |
|------|----------|
| **ConstFold** | Fold int/bool/float literals and pure unary/binary ops |
| **ConstProp** | SSA-like within straight-line HIR blocks / basic expression trees |
| **DCE** | Remove pure bindings never used (respect side-effecting calls, I/O, ARC ops as effectful) |
| **Pipeline driver** | Ordered list; fixed-point max N iterations (e.g. 5) |

**Files (expected):**

- `compiler/crates/ori-hir/src/optimize/{mod,pipeline,const_fold,dce}.rs`
- Hook in codegen/driver path after monomorph, before `emit_native`
- Unit tests for each pass

**Acceptance:**

- [ ] `cargo test -p ori-hir` covers fold/DCE
- [ ] `cargo test -p ori-driver --test ori_spec` (or existing suite) green
- [ ] No polyglot regression on `list_sum` / `fib_iter` (±10% noise)
- [ ] At least one golden case where folded HIR removes a redundant bind

**Risk:** medium — effectfulness model for DCE must treat runtime calls as opaque.

---

### Wave 2 — Loop hygiene + check elimination (1–2 PRs)

Target: reduce work **per iteration** in `while` loops that the typechecker already constrains.

| Technique | Rule (conservative) |
|-----------|---------------------|
| **IV simplify** | Recognize `i = 0; while i < n { …; i = i + 1 }` patterns |
| **Bounds elision** | `xs[i]` when `i` is IV and `0 ≤ i < lists.len(xs)` proven in-loop — emit unchecked load **or** single check outside |
| **Overflow policy** | Document: `int` ops are wrapping i64 in native (match polyglot parity); ensure lower uses wrapping add where language says so |
| **No-op ARC** | Never insert retain/release on pure `int`/`bool`/`float` (audit) |

**Acceptance:**

- [ ] `fib_iter` improves ≥ **2×** vs Wave 0 baseline (absolute wall time)
- [ ] `list_sum` no regression
- [ ] New regression tests: out-of-bounds still traps when not proven safe
- [ ] Spec / catalog unchanged unless a diagnostic is added for opt debug only

**Risk:** high if bounds elision is wrong — **prove or don’t elide**. Prefer missing opt over wrong opt.

---

### Wave 3 — Strength reduction + simple algebraic (1 PR)

| Pattern | Rewrite |
|---------|---------|
| Sum of IV `s += i` for `i in 0..n` | Closed form `n*(n-1)/2` when `s` starts 0 and loop is pure |
| `i * 2` / power-of-two | Shift when types are int |
| Nested `s += 1` for `i,j in 0..n` | `s = n*n` when pure |

**Acceptance:**

- [ ] `sum_loop` / `nested` either match closed form order-of-magnitude of C **or** documented “still loop, improved by ≥3×”
- [ ] Tests prove purity side conditions (I/O inside loop blocks rewrite)
- [ ] Update performance guide if closed form is intentional product behavior

**Risk:** medium — only pure loops; no rewrite across calls with effects.

---

### Wave 4 — Inlining monomorphic leafs (1–2 PRs)

| Target | Examples |
|--------|----------|
| Thin stdlib wrappers | Layer 2 `.orl` that only call Layer 1 |
| User `fn` leafs | Small functions (≤ N HIR nodes / ≤ M calls), monomorphic, not recursive |

Policy:

- Inline budget per call site and per function size.
- Never inline across crate/package boundaries in v1 if it breaks separate compilation assumptions (start **same module / same package** only).

**Acceptance:**

- [ ] Microbench or unit: call overhead of empty leaf drops
- [ ] `list_sum` stable or improved
- [ ] No binary size explosion on `examples/language_features` (optional size check ±20%)

**Risk:** medium — debug info / spans must remain honest enough for diagnostics.

---

### Wave 5 — Runtime list polish (optional, 1 PR)

Only if Wave 2–4 leave `list_sum` as the next cliff or real apps regress:

- `lists.reserve` / capacity growth documentation
- Reduce realloc churn in `ori_list_push`
- Optional `with_capacity` surface (**additive**, FREEZE-safe if purely additive API)

**Acceptance:** list microbench + existing multifile_imports green; CHANGELOG additive API.

---

### Wave 6 — Productization (1 PR)

| Item | Action |
|------|--------|
| Docs | Refresh `docs/guides/performance.md` EN+PT, README snapshot |
| Baseline | Append Wave 5/6 table to `perf-baseline-2026-07-13.md` |
| CHANGELOG | User-visible perf notes under Unreleased / next patch |
| BACKLOG | Mark `LANG-PERF-2` waves done; leave living residual |
| Polyglot default | Document `SAMPLES=5` for release notes |

---

## 6. PR Plan (ordered, mergeable)

| PR | Title | Depends | Scope | Est. |
|----|-------|---------|-------|------|
| **PR0** | `perf: emit CLIF dump + polyglot smoke harness` | — | Wave 0 | S |
| **PR1** | `perf(midend): pipeline skeleton + const fold + DCE` | PR0 | Wave 1 | M |
| **PR2** | `perf(midend): const prop + block CSE` | PR1 | Wave 1b | M |
| **PR3** | `perf(native): loop IV hygiene + safe bounds elision` | PR1 | Wave 2 | L |
| **PR4** | `perf(midend): pure-loop strength reduction` | PR2, PR3 | Wave 3 | M |
| **PR5** | `perf(midend): monomorphic leaf inlining` | PR2 | Wave 4 | M |
| **PR6** | `perf(runtime): list reserve path (optional)` | PR3 | Wave 5 | S–M |
| **PR7** | `docs: performance snapshot after LANG-PERF-2` | PR3+ | Wave 6 | S |

Each PR must:

1. `cargo test -p ori-driver` subset or workspace as appropriate  
2. `ori-testing` skill levels for any behavior change (check → compile → run)  
3. Not break FREEZE-1 surface  
4. Prefer **additive** diagnostics only  

Parallelism: PR5 can start after PR2 if PR3 is slow; PR4 should wait for PR3 so loop shape is stable.

---

## 7. Key decisions

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | Mid-end on **HIR**, not new SSA in wave 1 | Faster delivery; HIR already typed; SSA later if needed |
| D2 | **Conservative** opts only (prove or don’t) | Wrong bounds elision is a security/correctness bug |
| D3 | Primary metric kernels: **fib_iter, list_sum, nested** | Avoid vanity closed-form sum races |
| D4 | Keep **Cranelift** as product backend for this plan | Separate conversation for ORC/LLVM (§12) |
| D5 | AOT Default opts on; JIT measured separately | JIT startup vs runtime tradeoff already exists (`opt_level none`) |
| D6 | No surface break under FREEZE-1 | Additive APIs only (e.g. reserve) |
| D7 | Extract `ori-midend` crate only if size/complexity demands | clean-code Rule of Three |
| D8 | Effectful ops are opaque to DCE/fold | I/O, FFI, ARC, atomics, locks |

---

## 8. Testing strategy (`ori-testing` + extra)

| Layer | What |
|-------|------|
| L1 unit | Pass unit tests in `ori-hir` (before/after HIR snippets) |
| L2 | `ori check` / `ori compile` on polyglot kernels |
| L3 | `ori compile` + run binary; compare stdout |
| Regression | `ori-driver` tests for bounds failures that must still fail |
| Perf | `tools/bench/polyglot` before/after each wave (local; not necessarily CI gate) |
| Catalog | No new hard errors unless intentional; `diagnostic_catalog` if codes added |

**CI policy (recommended):**

- PR0 smoke script **optional** job or `daily_full` only (machines vary).
- Do **not** fail CI on absolute ms thresholds (host variance).
- Do fail CI on **correctness** tests.

---

## 9. Risks & mitigations

| Risk | Mitigation |
|------|------------|
| Incorrect bounds elision | Proof obligations + negative tests; default off until proven pattern matcher is solid |
| DCE drops needed ARC | Treat all managed retain/release as effects; audit list/string paths |
| Compile-time regression | Cap fixed-point iterations; mid-end disabled under `ORI_OPT=none` |
| Scope creep into LLVM | Hard non-goal; §12 only after waves 0–3 land |
| `native_backend.rs` 15kLOC | Loop work may touch it — small focused patches + tests; avoid drive-by refactors |
| FREEZE-1 pressure | Additive only; document in CHANGELOG as performance, not language change |

---

## 10. Effort & sequencing (calendar sketch)

Assuming one focused implementer familiar with the monorepo:

| Wave | Effort | Calendar (rough) |
|------|--------|------------------|
| 0 | 1–2 d | week 1 |
| 1 | 3–5 d | week 1–2 |
| 2 | 5–8 d | week 2–3 |
| 3 | 2–4 d | week 3–4 |
| 4 | 3–5 d | week 4–5 |
| 5 | 0–3 d | optional |
| 6 | 1 d | after measurable wins |

**Stop rule:** if after Wave 2 `fib_iter` has not improved ≥2×, pause and re-dump CLIF before Wave 3–4 (wrong bottleneck).

---

## 11. Tracking

| Artifact | Role |
|----------|------|
| This file | Normative plan for LANG-PERF-2 |
| `docs/planning/BACKLOG.md` | IDs `LANG-PERF-2` / wave rows |
| `docs/planning/perf-baseline-2026-07-13.md` | Measured tables |
| `docs/guides/performance.md` | User-facing snapshot |
| `tools/bench/polyglot/` | Repro harness |

### BACKLOG IDs (to add when plan is accepted)

| ID | Wave | P | D |
|----|------|---|---|
| LANG-PERF-2-0 | Instrument | 1 | S |
| LANG-PERF-2-1 | Mid-end fold/DCE | 1 | M |
| LANG-PERF-2-2 | Loop / bounds | 1 | L |
| LANG-PERF-2-3 | Strength reduce | 2 | M |
| LANG-PERF-2-4 | Inline leafs | 2 | M |
| LANG-PERF-2-5 | List reserve (opt) | 3 | S |
| LANG-PERF-2-6 | Docs snapshot | 2 | S |

---

## 12. Explicitly deferred: ORC (and friends)

**ORC** here means **LLVM ORC** (On-Request Compilation) — LLVM’s JIT infrastructure — *not* a new Ori language feature.

This plan **does not** implement ORC. After Waves 0–3 (or earlier if you prefer), we will discuss:

| Option | Idea | When it might make sense |
|--------|------|---------------------------|
| **A. Stay Cranelift-only** | Keep investing mid-end + Cranelift | Default if Wave 2–3 hit G1–G3 |
| **B. LLVM AOT backend** | Second backend for `ori compile` | If Cranelift plateaus on opts |
| **C. LLVM ORC JIT** | Replace/augment Cranelift JIT for `ori run` | If JIT runtime quality matters more than cold start |
| **D. Hybrid** | Cranelift JIT (fast start) + LLVM AOT (release) | Common industry pattern; highest maintenance |

**Discussion agenda (next conversation):**

1. Product goal: optimize **AOT binaries**, **`ori run` scripts**, or both?  
2. Acceptable cost: compile-time, binary size, dependency on LLVM, CI complexity.  
3. Parity tax: two backends → dual test matrix (already partial with C debug).  
4. FREEZE-1 / 0.3.x: backend swap can stay under the hood if ABI and semantics hold.  
5. Whether mid-end HIR is **backend-agnostic** (it should be — D1 supports B/C later).

No decision is made in this document.

---

## 13. Open questions (for plan acceptance)

| # | Question | Default if unanswered |
|---|----------|----------------------|
| Q1 | Accept LANG-PERF-2 into active BACKLOG now? | Yes — user asked for full plan to implement |
| Q2 | JIT: run Default mid-end or None? | Measure in PR1; start with **same as AOT Default** if startup OK |
| Q3 | Bounds elision aggressiveness | **Conservative** (D2) until proven patterns only |
| Q4 | Closed-form sum as intentional product opt? | Yes under Aggressive; Default may include if pure and tested |
| Q5 | Start ORC discussion immediately after plan merge? | Yes — user already queued that conversation |

---

## 14. Definition of done (LANG-PERF-2)

Plan is **done** when:

1. PR0–PR3 merged (minimum); PR4–PR7 as capacity allows.  
2. Goals G1–G3 measured on the same harness host class (or documented shortfall with root cause).  
3. Performance guide + baseline updated.  
4. No FREEZE-1 break; `cargo test --workspace` green.  
5. ORC decision recorded in a short ADR or BACKLOG note (accept or shelve).

---

## 15. Key Decisions (summary)

1. **HIR mid-end before any backend swap.**  
2. **fib / list / nested** are the scoreboard.  
3. **Correctness over cleverness** on bounds.  
4. **Cranelift stays** for this plan; **ORC is next discussion, not this work.**  
5. **Incremental PRs** with polyglot remeasure each wave.

---

## PR Plan (compact)

See §6 table **PR0–PR7**. Implement in order; stop-rule after Wave 2 if metrics stall.
