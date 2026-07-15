# ECO packages status — Ori sibling ports (`ori-*`)

> **Status:** active (2026-07-15)  
> **Linux-5 core stack:** **complete** (raylib…harfbuzz).  
> **W10 maturity-5 (U1–U15):** **done** — all packages **5 (Linux)** at **0.2.0**.  
> **Plan:** [`pr-plan-eco-maturity-5.md`](pr-plan-eco-maturity-5.md) — **PRs 1–19 complete** (W10 + Phase OS note).  
> **Policy (2026-07-15):** **implement / mature / port libs on Linux first.**  
> Multi-OS validation (Windows/mac) is **last** — scripts may exist, but execution is deferred.  
> **Canonical paths:** `/home/raillen/Documentos/Projetos/game-engine-full/ori-*`  
> **Matrix:** [`game-ports-maturity-matrix.md`](game-ports-maturity-matrix.md) ·  
> **Catálogo de ports (canônico):** [`eco-library-ports-catalog.md`](eco-library-ports-catalog.md) ·  
> **Roadmap:** `ori-game/docs/planning/ROADMAP-GAME-ECO.md`

---

## Score **5 (Linux)** gate (brief)

A package is **5 (Linux)** when plan §3 **G1–G7** hold on Linux:

| # | Criterion |
|---|-----------|
| **G1** | Broad product API (not smoke-only) — checklist in plan §4 |
| **G2** | ≥4 automated tests (or ≥3 if surface is tiny); `ori test` 0 failed |
| **G3** | Green `tools/smoke_linux.sh` (prints `ok`) |
| **G4** | README documents API + Phase OS note |
| **G5** | CHANGELOG entry for the maturity bump |
| **G6** | No dual path-dep leaf collision |
| **G7** | Version bump in `ori.pkg.toml` |

**Not required:** multi-OS, Marketplace, windowed demos, Studio. Full text: plan §3.

---

## Already **5 (Linux)** — do not re-implement

| Repo | Package | Ver. | Role |
|------|---------|------|------|
| **`ori-raylib`** | `raylib` | **0.1.0** | L0 raylib bindings (`ori_rl_*` shim) |
| **`ori-game`** | `ori_game` | **0.3.0** | L1 `game.*` + wires (`gltf`/`obj`/`physfs_assets`/`noise`/`compress`/`navmesh`) — PR 17 done |
| **`ori-imgui`** | `imgui` | **0.4.0** | Dear ImGui + Tier0/1 + optional raylib embed |
| **`ori-raygui`** | `raygui` | **0.2.0** | Immediate UI on raylib |
| **`ori-box2d`** | `box2d` | **0.3.0** | Box2D 3.x milli-unit physics |
| **`ori-jolt`** | `jolt` | **0.2.0** | Jolt 3D physics |
| **`ori-rres`** | `rres` | **0.3.0** | ORPK resource packs |
| **`ori-sqlite`** | `sqlite` | **0.3.0** | SQLite + prepared/multi-row |
| **`ori-enet`** | `enet` | **0.3.0** | ENet multiplayer (channels/protocol) |
| **`ori-freetype`** | `freetype` | **0.1.0** | FreeType face + text + gray atlas |
| **`ori-harfbuzz`** | `harfbuzz` | **0.1.0** | shape/layout + AOT tests (needs FreeType) |
| **`ori-stb`** | `stb` | **0.2.0** | image / perlin / rect_pack (U1) |
| **`ori-noise`** | `noise` | **0.2.0** | FastNoiseLite (U2) |
| **`ori-miniz`** | `miniz` | **0.2.0** | deflate / CRC / zip one-entry (U3) |
| **`ori-lz4`** | `lz4` | **0.2.0** | block + stream compress (U4) |
| **`ori-nfd`** | `nfd` | **0.2.0** | portable file dialogs (U5) |
| **`ori-implot`** | `implot` | **0.2.0** | ImPlot series + FULL (U6) |
| **`ori-imnodes`** | `imnodes` | **0.2.0** | node graph + FULL (U7) |
| **`ori-imguizmo`** | `imguizmo` | **0.2.0** | gizmo TRS + FULL (U8) |
| **`ori-tracy`** | `tracy` | **0.2.0** | zones/frames/plot/message (U9) |
| **`ori-enkiTS`** | `enkits` | **0.2.0** | task scheduler (U10) |
| **`ori-cgltf`** | `cgltf` | **0.2.0** | glTF 2.0 + mesh export (U11) |
| **`ori-fast-obj`** | `fast_obj` | **0.2.0** | Wavefront OBJ + flatten (U12) |
| **`ori-physfs`** | `physfs` | **0.2.0** | virtual FS write/multi-mount (U13) |
| **`ori-clay`** | `clay` | **0.2.0** | IM layout + command export (U14) |
| **`ori-recast`** | `recast` | **0.2.0** | navmesh triangle soup (U15) |

Bugfix-only touch-ups allowed if a dependent PR needs them.

---

## U1–U15 — **done** → **5 (Linux)** at **0.2.0**

All rows below met plan §3 G1–G7 (PRs 2–16). Listed for historical ID mapping only — do **not** re-open as maturity work.

| ID | Repo | Package | Ver. | Maturity | Plan PR |
|----|------|---------|------|----------|---------|
| **U1** | `ori-stb` | `stb` | **0.2.0** | **5 (Linux)** | PR 2 |
| **U2** | `ori-noise` | `noise` | **0.2.0** | **5 (Linux)** | PR 3 |
| **U3** | `ori-miniz` | `miniz` | **0.2.0** | **5 (Linux)** | PR 4 |
| **U4** | `ori-lz4` | `lz4` | **0.2.0** | **5 (Linux)** | PR 5 |
| **U5** | `ori-nfd` | `nfd` | **0.2.0** | **5 (Linux)** | PR 6 |
| **U6** | `ori-implot` | `implot` | **0.2.0** | **5 (Linux)** | PR 7 |
| **U7** | `ori-imnodes` | `imnodes` | **0.2.0** | **5 (Linux)** | PR 8 |
| **U8** | `ori-imguizmo` | `imguizmo` | **0.2.0** | **5 (Linux)** | PR 9 |
| **U9** | `ori-tracy` | `tracy` | **0.2.0** | **5 (Linux)** | PR 10 |
| **U10** | `ori-enkiTS` | `enkits` | **0.2.0** | **5 (Linux)** | PR 11 |
| **U11** | `ori-cgltf` | `cgltf` | **0.2.0** | **5 (Linux)** | PR 12 |
| **U12** | `ori-fast-obj` | `fast_obj` | **0.2.0** | **5 (Linux)** | PR 13 |
| **U13** | `ori-physfs` | `physfs` | **0.2.0** | **5 (Linux)** | PR 14 |
| **U14** | `ori-clay` | `clay` | **0.2.0** | **5 (Linux)** | PR 15 |
| **U15** | `ori-recast` | `recast` | **0.2.0** | **5 (Linux)** | PR 16 |

Content modules in **`ori-game`:** `game.tiled`, `game.ldtk`, `game.aseprite`, `game.spine`, `game.rres_assets`, `game.marching_cubes` (+ `marching_cubes_draw`), `game.gltf` / `game.obj` / `game.physfs_assets` / `game.noise` / `game.compress` / `game.navmesh` (**PR 17 wires done**).

---

## Layout (`game-engine-full`)

ECO game packages live under **`/home/raillen/Documentos/Projetos/game-engine-full/`** (model A: one folder, **N git remotes**).  
`ori-lang` (compiler docs for this inventory) and `ori-game-studio` stay **siblings** of that folder under `Projetos/` — not inside the cluster.

```
Documentos/Projetos/
  ori-lang/                    # compiler (outside cluster)
  ori-game-studio/             # Tauri app (outside cluster)
  game-engine-full/            # ECO game libs — each keeps own git remote
    ori-raylib/                # L0
    ori-game/                  # L1 hub (path-dep → siblings)
    ori-box2d/  ori-jolt/  ori-recast/
    ori-imgui/  ori-raygui/  ori-clay/
    ori-implot/ ori-imnodes/ ori-imguizmo/
    ori-freetype/ ori-harfbuzz/
    ori-rres/ ori-cgltf/ ori-fast-obj/ ori-physfs/
    ori-stb/ ori-noise/ ori-miniz/ ori-lz4/
    ori-enet/ ori-sqlite/ ori-enkiTS/ ori-tracy/ ori-nfd/
```

Path deps stay sibling-relative (`../ori-raylib`, …) inside `game-engine-full/`.

```toml
[dependencies]
raylib   = { path = "../ori-raylib", version = "0.1.0" }
ori_game = { path = "../ori-game", version = "0.3.0" }
imgui    = { path = "../ori-imgui", version = "0.4.0" }
box2d    = { path = "../ori-box2d", version = "0.3.0" }
enet     = { path = "../ori-enet", version = "0.3.0" }
```

---

## Smoke (Linux)

Umbrella script lives in **ori-game** and treats `proj_root` as the **parent of ori-game** (= `game-engine-full/`):

```bash
export ORI_BIN=$(command -v ori) ORI_USE_SYSTEM_LINKER=1
~/Documentos/Projetos/game-engine-full/ori-game/tools/smoke_eco_linux.sh
```

Packages listed (each `run_pkg_smoke` under `$proj_root/ori-*`):

- **Core:** raylib, game, box2d, jolt, imgui, raygui, rres, sqlite, enet  
- **High / U-ports:** freetype, harfbuzz, stb, noise, miniz, nfd, implot, imnodes, imguizmo, tracy, enkiTS  
- **Medium / U-ports:** cgltf, fast-obj, physfs, clay, **lz4**, recast  

Missing package dirs or smoke scripts are **auto-SKIP** (count as skip, not fail).

### `ECO_SMOKE_SKIP_*` env flags

| Variable | Effect |
|----------|--------|
| `ECO_SMOKE_SKIP_GAME=1` | Skip full `ori-game` smoke (ports-only run) |
| `ECO_SMOKE_SKIP_DEMOS=1` | Skip integration demos (`box2d_visual`, `jolt_boxes_3d`, …) |

Example (ports only, no demos):

```bash
ECO_SMOKE_SKIP_GAME=1 ECO_SMOKE_SKIP_DEMOS=1 \
  ~/Documentos/Projetos/game-engine-full/ori-game/tools/smoke_eco_linux.sh
```

---

## Phase OS (last — **non-blocking**)

**Policy:** do **not** block lib work or multi-OS CI green on Windows/mac.

| Tier | Scripts | Status |
|------|---------|--------|
| Core (game, box2d, jolt, sqlite, rres, imgui, raygui, enet) | real/stub `build_windows.ps1` + smoke | scripts ready — execute on MSVC host |
| U1–U15 (all **5 Linux** @ 0.2.0) | **deferred** `tools/build_windows.ps1` (echo only) | Linux-complete; multi-OS last |
| (legacy M1–M6 labels) | same as U4/U11–U15 | absorbed into maturity-5 |

Canonical write-up: [`PHASE-OS.md`](PHASE-OS.md). Umbrella: `ori-game/tools/smoke_eco_windows.ps1` (core only).  
**Maturity-5 plan residual:** **none** — PR 19 Phase OS note refresh **done**.

---

## Next work (Linux-only)

### Maturity-5 plan — **complete**

**Plan status:** [`pr-plan-eco-maturity-5.md`](pr-plan-eco-maturity-5.md) — **PRs 1–19 complete** (U1–U15 + ori-game wires + catalog/matrix/status + Phase OS note).

Prior ports plan [`pr-plan-eco-ports-e2e.md`](pr-plan-eco-ports-e2e.md): **PRs 1–10 complete** (0.1.0 scaffolds; do not re-scaffold).  
Catalog: [`eco-library-ports-catalog.md`](eco-library-ports-catalog.md)

Residual / roadmap (2026-07-15):
1. **Maturity U1–U15 → 5 (Linux)** — **done** (W10).  
2. **ImGui tools residual (P0–P3) → 5** — **executing** (`a68f7529`) · product source  
   `game-engine-full/ori-game/docs/planning/ROADMAP-GAME-ECO.md` **Next work** · execute-plan  
   [`pr-plan-imgui-tools-maturity-5.md`](pr-plan-imgui-tools-maturity-5.md) (stages A–E; residual §4).  
3. **`ori-miniaudio` skipped** — `game.audio` covers gap.  
4. Studio app = separate product track (`ori-game-studio`)  
5. Phase OS **execution** on MSVC = **last** (scaffolding done; non-blocking)

**Do not re-queue W10 engine ports.** Next stage = ImGui tools plan only.

**ECS:** no flecs/EnTT as default — see catalog §7 / roadmap § ECS.

---

## Implementation matrix

Full history: [`game-ports-maturity-matrix.md`](game-ports-maturity-matrix.md).
