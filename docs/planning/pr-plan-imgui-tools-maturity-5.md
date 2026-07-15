# Design + PR Plan — ImGui tools residual → maturity **5 (Linux)**

**Status:** **executing** — `/execute-plan` plan id `a68f7529` (PR1 lock-in **done**)  
**Date:** 2026-07-15  
**Source of truth (product backlog):**  
`game-engine-full/ori-game/docs/planning/ROADMAP-GAME-ECO.md` (ImGui extensions + packaging)  
**Cluster:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-*`  
**Policy:** Linux first. **Phase OS last** (non-blocking).  
**Maturity gate:** same **G1–G7** as [`pr-plan-eco-maturity-5.md`](pr-plan-eco-maturity-5.md) §3 (API + ≥4 tests + smoke + README + CHANGELOG + unique modules + version).  
**Prior plan (engine packages U1–U15):** **complete** — do not re-port nfd/implot/imnodes/imguizmo TRS/stb/cgltf/….  
**Linked from:** [`docs/planning/README.md`](README.md) · status Next work · catalog header · ROADMAP-GAME-ECO Next work  

**Execute:**

```bash
/execute-plan docs/planning/pr-plan-imgui-tools-maturity-5.md --concurrency 4
```

**User instructions (inject):**

```text
Linux only. Paths under /home/raillen/Documentos/Projetos/game-engine-full/.
Canonical product backlog: ori-game/docs/planning/ROADMAP-GAME-ECO.md.
Each new package = own git repo under game-engine-full/. Sibling path-deps.
int64_t ABI; milli-float where needed. Avoid Ori param name `len` (use nbytes).
Smoke ok; ori test 0 failed. Score 5 = G1–G7. Dual headless+FULL when ImGui C++ needed.
Do not re-implement packages already 5 (ori-nfd, ori-implot, ori-imnodes, ori-imguizmo TRS baseline, …).
Phase OS last. No flecs/miniaudio dual-stack. Prefer pure Ori for UX widgets when C++ not required.
```

---

## 1. Goal

Close the **ImGui / Studio-tools residual** from ROADMAP-GAME-ECO that is still open after W10:

1. **Stage A (P0 residual)** — in-UI file dialog (complement to nfd).  
2. **Stage B (P1)** — ImGuizmo suite extras + sequencer + multi-context/image + UX power tools + TexInspect.  
3. **Stage C (P2)** — ColorTextEdit, accessory widgets, memory editor, UI test hooks.  
4. **Stage D (P3)** — ImPlot3D, markdown (and document IME as Win-only Phase OS).  
5. **Stage E** — ROADMAP + catalog/status sync; Phase OS notes last.

One `/execute-plan` run drives **all stages** end-to-end without re-prompting between stages.

**Out of scope:** Tauri Studio product UI, flecs/EnTT, miniaudio, OpenAL, ozz, cute_c2, Hello ImGui / ImRAD / ImTui / remoting (stay **defer** unless a later plan reopens them).

---

## 2. Baseline — already **done** (do not re-implement)

| Item | Location | Notes |
|------|----------|--------|
| Dear ImGui + docking + tables + raylib embed | `ori-imgui` 0.4.0 | Tier0/1/2 MVP |
| nfd | `ori-nfd` 0.2.0 | OS dialogs |
| imnodes | `ori-imnodes` 0.2.0 | Lightweight nodes |
| ImPlot | `ori-implot` 0.2.0 | 2D charts |
| ImGuizmo TRS | `ori-imguizmo` 0.2.0 | translate/rotate/scale |
| Pure-Ori MVPs | `imgui.file_browser`, `inspector`, `nodes`, `plot`, `curves`, `timeline` | Not full upstream |
| Engine ports U1–U15 | stb…recast | W10 complete |

---

## 3. Score **5 (Linux)** — G1–G7 (same contract as maturity-5 plan)

| # | Criterion |
|---|-----------|
| **G1** | Broad product API per row in §4 |
| **G2** | ≥4 tests (or ≥3 if surface tiny), 0 failed |
| **G3** | `tools/smoke_linux.sh` green (headless; FULL `.a` when C++ ImGui client) |
| **G4** | README + Phase OS note |
| **G5** | CHANGELOG |
| **G6** | Unique module leaves across path-deps |
| **G7** | Version in `ori.pkg.toml` (new packages start **0.1.0** if already G1–G7 complete, or ship **0.2.0** after first deepen) |

**Not required:** multi-OS, Studio Tauri screens, windowed demos.

---

## 4. Residual inventory → deliverables

### Stage A — P0 residual

| ID | Deliverable | Upstream | G1 minimum (product surface) |
|----|-------------|----------|------------------------------|
| **A1** | **`ori-imguidialog`** (new) | [ImGuiFileDialog](https://github.com/aiekick/ImGuiFileDialog) | open/save UI path result; filter ext; cancel; headless test hooks; dual FULL if needs ImGui draw |

### Stage B — P1 (priority tools)

| ID | Deliverable | Upstream / approach | G1 minimum |
|----|-------------|---------------------|------------|
| **B1** | **`ori-imguizmo` deepen → 0.3.0** | ImCurveEdit + ImGradient (+ ImZoomSlider) same author | edit curve sample; gradient stops; zoom slider value; keep TRS |
| **B2** | **`ori-imsequencer`** (new) **or** deepen `imgui.timeline` to product 5 | [ImSequencer](https://github.com/ocornut/imgui/wiki/Useful-Extensions) / Cedric ecosystem | tracks, playhead milli, add/remove key; prefer **package** if C++ sequencer used |
| **B3** | **`ori-imgui` multi-context + image** | core + TextureId draw | create/set/destroy context (if gaps); `image` / `image_button` from texture id; document editor vs game contexts |
| **B4** | **`ori-imgui-extras`** (new) | pure Ori + thin host where needed: notify, search, hotkey, command palette, metrics | toast queue; filter list; capture hotkey string; palette run command; fps/frame milli display |
| **B5** | **`ori-imgui-texinspect`** (new) or module in extras | [ImGuiTexInspect](https://github.com/ocornut/imgui/wiki/Useful-Extensions) or thin Ori | show texture id; zoom; channel toggle MVP |

### Stage C — P2

| ID | Deliverable | Upstream / approach | G1 minimum |
|----|-------------|---------------------|------------|
| **C1** | **`ori-imgui-textedit`** (new) | [ColorTextEdit pthom](https://github.com/pthom/ImGuiColorTextEdit/tree/imgui_bundle) | set/get text; highlight lang stub; headless buffer API + FULL draw when linked |
| **C2** | **`ori-imgui-widgets`** (new) or extras modules | knobs, toggle, spinner, spectrum theme helpers | ≥1 of each widget API + style load MVP; pure Ori preferred |
| **C3** | **`ori-imgui-memory`** (new) | [imgui_memory_editor](https://github.com/ocornut/imgui_club) | bind byte buffer; read/write cell; smoke without GUI hang |
| **C4** | **UI test hooks** | imgui_test_engine *or* pure Ori harness | Prefer **pure Ori** `imgui.test_harness` (click/query by label) unless C++ engine is low cost; document choice |

### Stage D — P3

| ID | Deliverable | Upstream / approach | G1 minimum |
|----|-------------|---------------------|------------|
| **D1** | **`ori-implot3d`** (new) | [ImPlot3D](https://github.com/brenocq/implot3d) | scatter/line 3D series milli; dual FULL; depends implot/imgui |
| **D2** | **Markdown in UI** | imgui_markdown **or** pure Ori markdown subset | render headings + paragraphs + code fence to ImGui text; package `ori-imgui-markdown` or module under extras |
| **D3** | **IME / CJK** | DearImGui-with-IMM32 | **Docs-only on Linux plan:** document Windows-only; implement stub API no-op on Linux; real Win in Phase OS |

### Explicitly deferred (do not PR in this plan)

ImNodeFlow · imGuIZMO.quat · Hello ImGui · ImRAD · ImTui · netImGui · imgui-ws · software renderer · DatePicker · ImAnim · Teselka hex · Zep · reflection auto-UI (ImRefl) · InAppGpuProfiler (use Tracy) · ImGuiFD (covered by nfd + FileDialog)

---

## 5. PR Plan (DAG)

### PR 1: ROADMAP lock-in + residual inventory (docs) — **done** (2026-07-15)

**Description:** Ensure `ROADMAP-GAME-ECO.md` reflects done P0 packages and points **Next work** at this plan. Sync short pointers in `ori-lang` status/catalog. No package code.

**Files/components affected:**  
`/home/raillen/Documentos/Projetos/game-engine-full/ori-game/docs/planning/ROADMAP-GAME-ECO.md`,  
`docs/planning/pr-plan-imgui-tools-maturity-5.md`,  
`docs/planning/eco-packages-status.md`,  
`docs/planning/README.md`

**Dependencies:** None

**Done notes:** ROADMAP **Next work** → this plan; status/README pointers set; residual inventory remains in this plan §4 (no long-table duplication). Plan status **executing** (`a68f7529`).

---

### PR 2: ori-imguidialog 0.1.0 → 5 (Linux) — Stage A / P0

**Description:** New package `ori-imguidialog` wrapping ImGuiFileDialog (or equivalent). Path-dep `imgui`. Headless tests for filter/cancel/path result; FULL draw path optional dual artifact. G1–G7. Smoke.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imguidialog/**` (new)

**Dependencies:** None

---

### PR 3: ori-imguizmo 0.3.0 — CurveEdit + Gradient + ZoomSlider — Stage B1

**Description:** Extend existing `ori-imguizmo` with ImCurveEdit, ImGradient, ImZoomSlider from same upstream family. Keep TRS. ≥4 new tests + smoke. Version **0.3.0**. G1–G7.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imguizmo/**`

**Dependencies:** None

---

### PR 4: ori-imsequencer 0.1.0 → 5 — Stage B2

**Description:** New package **or** if pure Ori is enough, expand `ori-imgui` `imgui.timeline` to G1–G7 product surface and **skip** new repo — **prefer package only if C++ ImSequencer is used**. Document choice in summary. Tracks + playhead milli + key CRUD; smoke.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imsequencer/**` **or** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imgui/**`

**Dependencies:** None

---

### PR 5: ori-imgui multi-context + image helpers — Stage B3

**Description:** Close gaps vs Multi-Context Compositor needs: ensure create/set/destroy/current context APIs are complete and tested; `image` / `image_button` from texture id (raylib/GL id as int). Version bump (e.g. 0.4.0 → 0.5.0). Smoke.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imgui/**`

**Dependencies:** None

---

### PR 6: ori-imgui-extras 0.1.0 → 5 — Stage B4 (P1 UX)

**Description:** New package (mostly pure Ori + minimal host):  
`notify` · `search` · `hotkey` · `command_palette` · `metrics`  
Path-dep `imgui` only. ≥4 tests covering queue/filter/palette/metrics. Smoke. Prefer pure Ori; C++ only if required.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imgui-extras/**` (new)

**Dependencies:** None

---

### PR 7: ori-imgui-texinspect 0.1.0 → 5 — Stage B5

**Description:** New package for texture inspect MVP (zoom, channels) or implement as modules under `ori-imgui-extras` if smaller — **prefer dedicated package if C++ ImGuiTexInspect**. G1–G7. Smoke.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imgui-texinspect/**` **or** `ori-imgui-extras/**`

**Dependencies:** None

---

### PR 8: ori-imgui-textedit 0.1.0 → 5 — Stage C1 / P2

**Description:** ColorTextEdit (pthom fork). Buffer set/get; language stub; dual FULL. G1–G7. Smoke.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imgui-textedit/**` (new)

**Dependencies:** None

---

### PR 9: ori-imgui-widgets 0.1.0 → 5 — Stage C2 / P2

**Description:** Knobs, toggle, spinner, spectrum theme helper (pure Ori preferred). If pure Ori fits entirely, modules may live under `ori-imgui-extras` — one package max. G1–G7. Smoke.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imgui-widgets/**` **or** `ori-imgui-extras/**`

**Dependencies:** None

---

### PR 10: ori-imgui-memory 0.1.0 → 5 — Stage C3 / P2

**Description:** imgui_memory_editor binding. Byte buffer view/edit API; headless tests. G1–G7. Smoke.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imgui-memory/**` (new)

**Dependencies:** None

---

### PR 11: imgui test harness — Stage C4 / P2

**Description:** Pure Ori `imgui.test_harness` (preferred) **or** thin wrap of imgui_test_engine. Query widgets by label; simulate click/open; ≥4 tests. Ship inside `ori-imgui` or `ori-imgui-extras`. Document non-goal: full official test engine CI.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imgui/**` **or** `ori-imgui-extras/**`

**Dependencies:** None

---

### PR 12: ori-implot3d 0.1.0 → 5 — Stage D1 / P3

**Description:** ImPlot3D package; path-dep implot + imgui. Series milli; dual FULL. G1–G7. Smoke.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-implot3d/**` (new)

**Dependencies:** None

---

### PR 13: markdown UI + IME policy — Stage D2–D3 / P3

**Description:**  
- Markdown: `ori-imgui-markdown` **or** extras module — headings, paragraphs, fenced code → ImGui. G1–G7.  
- IME: Linux no-op stubs + README that real IMM32 is Phase OS Windows-only.

**Files/components affected:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-imgui-markdown/**` **or** `ori-imgui-extras/**`, optionally `ori-imgui/**` for IME stubs

**Dependencies:** None

---

### PR 14: Catalog + matrix + ROADMAP + umbrella smoke

**Description:** Mark all new packages **5 (Linux)** in ori-lang catalog/status/matrix. Update ROADMAP-GAME-ECO residual tables to **done**. Extend `smoke_eco_linux.sh` for new packages (SKIP if missing). Plan status → stages complete except Phase OS.

**Files/components affected:**  
`docs/planning/eco-library-ports-catalog.md`,  
`docs/planning/eco-packages-status.md`,  
`docs/planning/game-ports-maturity-matrix.md`,  
`docs/planning/pr-plan-imgui-tools-maturity-5.md`,  
`/home/raillen/Documentos/Projetos/game-engine-full/ori-game/docs/planning/ROADMAP-GAME-ECO.md`,  
`/home/raillen/Documentos/Projetos/game-engine-full/ori-game/tools/smoke_eco_linux.sh`

**Dependencies:** PR 2, PR 3, PR 4, PR 5, PR 6, PR 7, PR 8, PR 9, PR 10, PR 11, PR 12, PR 13

---

### PR 15: Phase OS scaffolding last (non-blocking)

**Description:** Phase OS section + deferred `build_windows.ps1` on every **new** package from this plan. Update `PHASE-OS.md`. No multi-OS CI green required.

**Files/components affected:** `docs/planning/PHASE-OS.md`, new package READMEs under `game-engine-full/ori-imgui-*/`, `docs/planning/eco-packages-status.md`

**Dependencies:** PR 14

---

## 6. Parallelism (levels)

| Level | PRs | Stage |
|-------|-----|-------|
| 0 | PR 1 + PR 2…PR 13 | Docs + all independent package/feature deepens |
| 1 | PR 14 | Catalog / ROADMAP / smoke after all deliverables |
| 2 | PR 15 | Phase OS last |

Linearized: `PR1 → PR2 → … → PR13 → PR14 → PR15`  
(Within level 0, PRs 2–13 run in parallel; concurrency 4 recommended.)

**Stage order for humans (same DAG):**

```text
Stage A  PR2          FileDialog
Stage B  PR3–PR7      ImGuizmo suite, sequencer, multi-ctx/image, extras, texinspect
Stage C  PR8–PR11     textedit, widgets, memory, test harness
Stage D  PR12–PR13    implot3d, markdown+IME policy
Stage E  PR14–PR15    docs + Phase OS
```

---

## 7. Packaging map (target layout)

```text
game-engine-full/
  ori-imgui/              # + multi-context/image (PR5)
  ori-imguizmo/           # + CurveEdit/Gradient/Zoom (PR3)
  ori-implot/             # already 5 — no PR
  ori-imnodes/            # already 5 — no PR
  ori-nfd/                # already 5 — no PR
  ori-imguidialog/        # PR2 NEW
  ori-imsequencer/        # PR4 NEW (or skipped if pure Ori timeline)
  ori-imgui-extras/       # PR6 (+ maybe widgets/harness)
  ori-imgui-texinspect/   # PR7 NEW (or extras)
  ori-imgui-textedit/     # PR8 NEW
  ori-imgui-widgets/      # PR9 NEW (or extras)
  ori-imgui-memory/       # PR10 NEW
  ori-implot3d/           # PR12 NEW
  ori-imgui-markdown/     # PR13 NEW (or extras)
```

Do **not** merge into `ori-game`. Studio path-deps later.

---

## 8. Implementer playbook

```bash
export ORI_BIN=/home/raillen/.grok/worktrees/projetos-ori-lang/game-engine-final/compiler/target/debug/ori
export ORI_RUNTIME_CDYLIB=…/libori_runtime.so
export ORI_RUNTIME_LIB=…/libori_runtime.a
export ORI_USE_SYSTEM_LINKER=1
export ORI_USE_JIT=1
```

- Absolute paths under `game-engine-full/`.  
- C++ ImGui clients: dual headless + FULL `.a` (see implot/imguizmo patterns).  
- Link with sibling `ori-imgui` when FULL.  
- Commit per package git root.  
- Summary: `/tmp/grok-$(id -u)/grok-exec-summary-<PLAN_ID>-pr-<n>.md`

---

## 9. Risk register

| Risk | Mitigation |
|------|------------|
| Scope explosion (full wiki) | Cap G1 to §4 rows only |
| ImGui C++ link hell | Dual artifact; smoke uses headless |
| Too many tiny packages | Allowed to fold pure-Ori into `ori-imgui-extras` (document in summary) |
| ColorTextEdit maintenance | pthom fork only |
| Sequencer C++ vs Ori timeline | Prefer pure Ori if G1 met without FFI |
| execute-plan isolation | Multi-repo absolute paths |

---

## 10. Success criteria

1. All Stage A–D deliverables meet G1–G7 on Linux (or documented pure-Ori fold into extras).  
2. ROADMAP-GAME-ECO residual P0/P1/P2/P3 items from §4 marked **done** (defer list remains defer).  
3. Catalog/status list new packages at **5 (Linux)**.  
4. `smoke_eco_linux.sh` includes new packages (SKIP if missing).  
5. Phase OS still last / non-blocking.  
6. No re-open of W10 engine packages.

---

## 11. Effort sketch

| Stage | PRs | Rough effort |
|-------|-----|--------------|
| A P0 | PR2 | 1–2 d |
| B P1 | PR3–7 | 5–8 d (parallel) |
| C P2 | PR8–11 | 4–6 d |
| D P3 | PR12–13 | 2–3 d |
| E | PR14–15 | 0.5–1 d |

Wall-clock with concurrency 4: ~1–2 weeks.

---

## Appendix — Acceptance snippet

```text
cd $PKG && ./tools/smoke_linux.sh   # smoke ok
ori test tests/…                     # N passed, 0 failed
maturity 5 (Linux) — G1..G7 met
```
