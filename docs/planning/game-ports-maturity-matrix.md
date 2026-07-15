# Game / ECO ports — maturity & backlog matrix

> **Status:** active consult doc (implementation reference)  
> **Updated:** 2026-07-15  
> **Program:** core Linux-5 **complete**; **W10** (U1–U15 → **5 Linux**) **done**; **ImGui tools A–D → 5 Linux** **done**.  
> **Maturity-5 plan:** [`pr-plan-eco-maturity-5.md`](pr-plan-eco-maturity-5.md) — **PRs 1–19 complete**.  
> **ImGui tools plan:** [`pr-plan-imgui-tools-maturity-5.md`](pr-plan-imgui-tools-maturity-5.md) — **PRs 1–15 complete** (`a68f7529`).  
> **Policy:** implement / mature / port libs **on Linux first**. Multi-OS (**Phase OS**) is **last**.  
> **Cluster path:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-*`  
> **Related:** [`eco-packages-status.md`](eco-packages-status.md) ·  
> [`eco-library-ports-catalog.md`](eco-library-ports-catalog.md) ·  
> [`pr-plan-eco-ports-e2e.md`](pr-plan-eco-ports-e2e.md) (medium ports 0.1.0 — complete) ·  
> `ori-game/docs/planning/ROADMAP-GAME-ECO.md`

### Maturity scale

| Score | Meaning |
|------:|---------|
| **1** | Skeleton / plan only |
| **2** | MVP links + minimal demo/smoke |
| **3** | Small real use / jam-viable on Linux |
| **4** | Broad surface + tests + several demos |
| **5** | Product engine-grade on **Linux** (this program’s target). Multi-OS = Phase OS |

### Score **5 (Linux)** gate (W10 / maturity-5 plan)

Package reaches **5** when plan §3 **G1–G7** hold (Linux only):

| # | Criterion |
|---|-----------|
| **G1** | Broad product API (checklist plan §4) |
| **G2** | ≥4 tests (or ≥3 if tiny surface); `ori test` 0 failed |
| **G3** | Green package `tools/smoke_linux.sh` |
| **G4** | README documents API + Phase OS note |
| **G5** | CHANGELOG for maturity bump |
| **G6** | No dual path-dep leaf collision |
| **G7** | Version bump in `ori.pkg.toml` |

**Not required for 5:** multi-OS, Marketplace, windowed demos, Studio. Full text: [`pr-plan-eco-maturity-5.md`](pr-plan-eco-maturity-5.md) §3.

---

## Wave progress (Linux-5 program)

| Wave | Focus | Status |
|------|--------|--------|
| **W1** | ori-game S3 demo hygiene + smoke expand | **done** |
| **W2** | ori-game gamepad + RenderTexture → 0.3.0 | **done** |
| **W3** | ori-box2d → 0.3.0 | **done** |
| **W4** | ori-jolt → 0.2.0 | **done** |
| **W5** | raygui 0.2.0 + imgui 0.3.0 | **done** |
| **W6** | rres 0.3.0 + sqlite 0.3.0 | **done** |
| **W7** | Matrix gate all **5 (Linux)** (core stack) | **done** |
| **W8** | Integration demos + umbrella smoke | **done** (2026-07-14) |
| **W9+** | Deepen + new ports (Linux) 0.1.0 | **done** — high+medium + ImGui T2 |
| **W10** | All ECO packages → **5 (Linux)** (U1–U15) | **done** (2026-07-15) — PRs 2–16 packages + PR 17 wires + PR 18 catalog |
| **W11** | ImGui tools residual A–D → **5 (Linux)** | **done** (2026-07-15) — plan `a68f7529` PRs 1–15 complete |
| **Phase OS** | Win/mac stage + smoke | **last** (scaffolding/docs done; not blocking) — multi-OS execution deferred |

---

## Table A — Already **5 (Linux)** (do not re-implement)

Paths under `game-engine-full/`.

| Package | Repo | Ver. | Maturity | Status |
|---------|------|------|----------|--------|
| `raylib` L0 | `ori-raylib` | **0.1.0** | **5 (Linux)** | split from ori-game |
| `ori_game` | `ori-game` | **0.3.0** | **5 (Linux)** | L1 `game.*` + content; wires PR 17 **done** |
| `box2d` | `ori-box2d` | **0.3.0** | **5 (Linux)** | joints, poly4, queries, contacts, materials |
| `jolt` | `ori-jolt` | **0.2.0** | **5 (Linux)** | layers, friction, torque, floor, hit body |
| `imgui` | `ori-imgui` | **0.5.1** | **5 (Linux)** | multi-ctx/image + timeline + `test_harness` (tools B2/B3/C4) |
| `raygui` | `ori-raygui` | **0.2.0** | **5 (Linux)** | textbox, toggle, dropdown, style, … |
| `rres` | `ori-rres` | **0.3.0** | **5 (Linux)** | validate, list_names, read_bytes |
| `sqlite` | `ori-sqlite` | **0.3.0** | **5 (Linux)** | prepared + multi-row JSON |
| `enet` | `ori-enet` | **0.3.0** | **5 (Linux)** | channels, broadcast, protocol |
| `freetype` | `ori-freetype` | **0.1.0** | **5 (Linux)** | face + text + gray atlas |
| `harfbuzz` | `ori-harfbuzz` | **0.1.0** | **5 (Linux)** | shape/layout + AOT tests |
| `stb` | `ori-stb` | **0.2.0** | **5 (Linux)** | U1 / PR 2 — image / perlin / rect_pack |
| `noise` | `ori-noise` | **0.2.0** | **5 (Linux)** | U2 / PR 3 — FastNoiseLite |
| `miniz` | `ori-miniz` | **0.2.0** | **5 (Linux)** | U3 / PR 4 — deflate / CRC / zip |
| `lz4` | `ori-lz4` | **0.2.0** | **5 (Linux)** | U4 / PR 5 — block + stream |
| `nfd` | `ori-nfd` | **0.2.0** | **5 (Linux)** | U5 / PR 6 — file dialogs |
| `implot` | `ori-implot` | **0.2.0** | **5 (Linux)** | U6 / PR 7 — series + FULL |
| `imnodes` | `ori-imnodes` | **0.2.0** | **5 (Linux)** | U7 / PR 8 — node graph + FULL |
| `imguizmo` | `ori-imguizmo` | **0.3.0** | **5 (Linux)** | U8 + tools B1 — TRS + CurveEdit/Gradient/Zoom |
| `tracy` | `ori-tracy` | **0.2.0** | **5 (Linux)** | U9 / PR 10 — zones/frames |
| `enkits` | `ori-enkiTS` | **0.2.0** | **5 (Linux)** | U10 / PR 11 — task scheduler |
| `cgltf` | `ori-cgltf` | **0.2.0** | **5 (Linux)** | U11 / PR 12 — glTF 2.0 |
| `fast_obj` | `ori-fast-obj` | **0.2.0** | **5 (Linux)** | U12 / PR 13 — Wavefront OBJ |
| `physfs` | `ori-physfs` | **0.2.0** | **5 (Linux)** | U13 / PR 14 — virtual FS |
| `clay` | `ori-clay` | **0.2.0** | **5 (Linux)** | U14 / PR 15 — IM layout |
| `recast` | `ori-recast` | **0.2.0** | **5 (Linux)** | U15 / PR 16 — navmesh MVP |
| `imguidialog` | `ori-imguidialog` | **0.1.0** | **5 (Linux)** | tools A1 — in-UI open/save |
| `imgui_extras` | `ori-imgui-extras` | **0.1.0** | **5 (Linux)** | tools B4 — notify/search/hotkey/palette/metrics |
| `texinspect` | `ori-imgui-texinspect` | **0.1.0** | **5 (Linux)** | tools B5 — texture inspect |
| `imtextedit` | `ori-imgui-textedit` | **0.1.0** | **5 (Linux)** | tools C1 — ColorTextEdit-style buffer |
| `widgets` | `ori-imgui-widgets` | **0.1.0** | **5 (Linux)** | tools C2 — knobs/toggle/spinner/theme |
| `immemory` | `ori-imgui-memory` | **0.1.0** | **5 (Linux)** | tools C3 — hex memory editor |
| `implot3d` | `ori-implot3d` | **0.1.0** | **5 (Linux)** | tools D1 — ImPlot3D |
| `markdown` | `ori-imgui-markdown` | **0.1.0** | **5 (Linux)** | tools D2–D3 — markdown + IME stubs |

## Table A2 — U1–U15 historical IDs (all **5 (Linux)** @ **0.2.0**)

Canonical API targets: [`pr-plan-eco-maturity-5.md`](pr-plan-eco-maturity-5.md) §4. Rows retained for ID → package mapping only.

| ID | Package | Repo | Ver. | Maturity | Plan PR | Notes |
|----|---------|------|------|----------|---------|-------|
| **U1** | `stb` | `ori-stb` | **0.2.0** | **5 (Linux)** | PR 2 | image / perlin / rect_pack |
| **U2** | `noise` | `ori-noise` | **0.2.0** | **5 (Linux)** | PR 3 | FastNoiseLite |
| **U3** | `miniz` | `ori-miniz` | **0.2.0** | **5 (Linux)** | PR 4 | deflate / CRC |
| **U4** | `lz4` | `ori-lz4` | **0.2.0** | **5 (Linux)** | PR 5 | LZ4 compress |
| **U5** | `nfd` | `ori-nfd` | **0.2.0** | **5 (Linux)** | PR 6 | file dialogs |
| **U6** | `implot` | `ori-implot` | **0.2.0** | **5 (Linux)** | PR 7 | series + FULL draw |
| **U7** | `imnodes` | `ori-imnodes` | **0.2.0** | **5 (Linux)** | PR 8 | node graph + FULL |
| **U8** | `imguizmo` | `ori-imguizmo` | **0.3.0** | **5 (Linux)** | PR 9 + tools B1 | TRS + CurveEdit/Gradient/Zoom |
| **U9** | `tracy` | `ori-tracy` | **0.2.0** | **5 (Linux)** | PR 10 | zones/frames + FULL |
| **U10** | `enkits` | `ori-enkiTS` | **0.2.0** | **5 (Linux)** | PR 11 | task scheduler |
| **U11** | `cgltf` | `ori-cgltf` | **0.2.0** | **5 (Linux)** | PR 12 | glTF 2.0 |
| **U12** | `fast_obj` | `ori-fast-obj` | **0.2.0** | **5 (Linux)** | PR 13 | Wavefront OBJ |
| **U13** | `physfs` | `ori-physfs` | **0.2.0** | **5 (Linux)** | PR 14 | virtual FS |
| **U14** | `clay` | `ori-clay` | **0.2.0** | **5 (Linux)** | PR 15 | IM layout |
| **U15** | `recast` | `ori-recast` | **0.2.0** | **5 (Linux)** | PR 16 | navmesh MVP |
| — | Studio | plan only | — | 0.5–1 | — | Separate product track |

### Detail surfaces (ori-game)

| Surface | Maturity |
|---------|----------|
| `game.app` | 5 |
| `game.input` (+ gamepad) | 5 |
| `game.draw` (+ RenderTexture) | 5 |
| `game.audio` | **5** (buses, pool, seek) |
| 2D systems (tilemap, particles, inventory, dialogue, scene, …) | **5** |
| 3D / shaders / light bank | **5** (presets; no shadow) |
| Mechanics | **5** (i-frames, patrol/aggro) |
| Wires (`gltf`/`obj`/`physfs_assets`/`noise`/`compress`/`navmesh`) | **5** (PR 17) |

---

## Table B — Remaining (post Linux-5)

### B1 — Phase OS (multi-OS) — **last** (non-blocking)

| ID | Item | Priority | Notes |
|----|------|----------|-------|
| **B1.4** | Stage Win (+ mac) libs for all ECO packages | **last** | Core scripts ready; U1–U15 + ImGui tools = deferred stubs ([`PHASE-OS.md`](PHASE-OS.md)) |
| **B1.4b** | smoke_windows / CI multi-OS | **last** | **Do not** require green for product progress |

Scaffolding: core real/stub scripts + U1–U15 + ImGui tools residual README Phase OS + `build_windows.ps1` deferred stubs — **done** (ports-e2e PR 10 + maturity-5 PR 19 + ImGui tools PR 15).

### B2 — Deepen + ports (Linux)

| ID | Item | Priority | Status |
|----|------|----------|--------|
| **B2.1** | Tiled + LDtk | P1 | **done** — `game.tiled` + `game.ldtk` |
| **B2.2** | enet / multiplayer | P1 | **done** — `ori-enet` **0.3.0** |
| **B2.3** | Aseprite + Spine | P1 | **done** — `game.aseprite` + `game.spine` |
| **B2.4** | 3D/audio deepen | P2 | **done** — pitch/pan/master; cylinder/capsule/billboard |
| **B2.5** | rres ↔ assets + physics debug draw | P2 | **done** — `game.rres_assets`; debug_draw |
| **B1.15** | ImGui inside raylib window | P2 | **done** — `imgui.init_raylib` |
| **B2.12** | Split `ori-raylib` | P3 | **done** — `raylib` 0.1.0 |
| **B2.13** | ImGui Tier 0 dock/tables | P2 | **done** |
| **B2.14** | ImGui Tier 1 file/plot/nodes | P2 | **done** — **0.4.0** |
| **B2.15** | ImGui multi-context (editor vs game) | P2 | **done** — create/set/destroy context |
| **B2.16** | ImGui Tier 2 (style, image, curves, timeline) | P2 | **done** MVP — pure Ori curves/timeline; style/image host |
| **B2.17** | Surface maturity 4→5 (3D/shaders, mechanics, audio edge) | P2 | **done** — buses/pool/seek; combat i-frames; fog presets; dialogue/inv |
| **B2.18** | New sibling ports (product-driven `ori-*`) | P2 | **done (high + medium 0.1.0)** — do not re-scaffold. Plan e2e: [`pr-plan-eco-ports-e2e.md`](pr-plan-eco-ports-e2e.md). Residual new port: `ori-miniaudio` only if gap (skipped). |
| **B2.18b** | **Maturity U1–U15 → 5 (Linux)** | P1 | **done** — Table A / A2; plan PRs 2–16 + PR 18 catalog (G1–G7). |
| **B2.18c** | **ImGui tools residual A–D → 5 (Linux)** | P1 | **done** — Table A tools rows; plan `a68f7529` **PRs 1–15 complete** |
| **B2.19** | In-`ori-game` exploration (camera, save, A\*, actions) | P2 | **done** — camera limits/shake; slots; pathfind; actions; cutscene; net_predict |
| **B2.20** | ECS (flecs/EnTT) | — | **declined as default** — optional only if measured need |

### B3 — Studio

| ID | Item | Priority |
|----|------|----------|
| **B3.1** | Ori Game Studio Tauri app | separate product track |

---

## Acceptance notes (Linux-5 definition used)

- Product surface for jam/mid-size games on **Linux**, not 100% C 1:1 parity.
- **W10 gate** for U1–U15: plan §3 **G1–G7** (API + tests + smoke + README + CHANGELOG + leaf + version) — **met** for all U1–U15.
- Multi-OS deferred by explicit user decision (2026-07-14); does **not** block score 5.
- Layout: all package code under `Documentos/Projetos/game-engine-full/ori-*` (not monorepo `packages/`).

## How to update

1. After Phase OS work: raise maturity to **5 (Linux+Win)** etc., clear B1 rows.  
2. New ports only with catalog product need — do not re-open U1–U15 or ImGui tools A–D maturity.
