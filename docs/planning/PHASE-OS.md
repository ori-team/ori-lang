# Phase OS — multi-OS staging for ECO packages

> **Status:** **scaffolding** (2026-07-15) — **non-blocking / last**  
> **Linux product surface is complete** for core + U1–U15 + **ImGui tools residual** packages.  
> Multi-OS validation (Windows/mac) is **last**: do **not** block lib work or CI on Win/mac green.  
> Core packages (game stack) have real/stub Windows scripts from 2026-07-14;  
> U1–U15 / ImGui tools residual have **deferred stubs only** (echo + exit 0) where present  
> (except **ori-imgui** MSVC host stub script — still not a multi-OS CI gate).  
> Plans: maturity-5 **PRs 1–19 complete**; ImGui tools **PRs 1–15 complete** (`a68f7529`).

## Goal

| Triple | Priority | Status |
|--------|----------|--------|
| `x86_64-unknown-linux-gnu` | done | **Linux complete** — core + U1–U15 + ImGui tools residual (**5 (Linux)**) |
| `x86_64-pc-windows-msvc` | **P0** (later) | core scripts ready; U/tools = **deferred stubs** |
| `x86_64-apple-darwin` / `aarch64-apple-darwin` | P1 (later) | not started |

## Prerequisites (Windows)

1. **Visual Studio 2022 Build Tools** (workload *Desktop development with C++*)
2. **cmake** on PATH (Box2D / optional real Jolt)
3. **ori.exe** ≥ 0.3.x with staged `runtime/x86_64-pc-windows-msvc/`
4. Shell: **x64 Native Tools Command Prompt** *or* scripts auto-load via `vswhere`

```powershell
$env:ORI_BIN = "C:\path\to\ori.exe"
$env:ORI_USE_SYSTEM_LINKER = "1"
```

## Naming (MSVC)

Package `native_libs = ["foo"]` → link looks for **`foo.lib`** under `lib/x86_64-pc-windows-msvc/`.

| Linux | Windows |
|-------|---------|
| `libori_raylib_shim.a` | `ori_raylib_shim.lib` |
| `libraylib.a` | `raylib.lib` |
| `libori_box2d_shim.a` | `ori_box2d_shim.lib` |
| `libbox2d.a` | `box2d.lib` |

## Per-package scripts (core — real/stub MSVC)

| Package | Build | Smoke |
|---------|-------|-------|
| **ori-game** | `tools/setup_raylib_windows.ps1` [`-Stub`] | `tools/smoke_windows.ps1` [`-Stub`] |
| **ori-box2d** | `tools/build_windows.ps1` | `tools/smoke_windows.ps1` |
| **ori-jolt** | `tools/build_windows.ps1` (stub default) | `tools/smoke_windows.ps1` |
| **ori-sqlite** | `tools/build_windows.ps1` | `tools/smoke_windows.ps1` |
| **ori-rres** | `tools/build_windows.ps1` | `tools/smoke_windows.ps1` |
| **ori-imgui** **0.5.1** | `tools/build_windows.ps1` (MSVC host stub) | `tools/smoke_windows.ps1` |
| **ori-raygui** | `tools/build_windows.ps1` | `tools/smoke_windows.ps1` |
| **ori-enet** | `tools/build_windows.ps1` | `tools/smoke_windows.ps1` |

## Maturity-5 packages (U1–U15) — Linux **complete**; Phase OS **deferred**

All U1–U15 packages are **5 (Linux)** at **0.2.0** (plan `pr-plan-eco-maturity-5.md` PRs 2–16),  
except **ori-imguizmo** deepened to **0.3.0** (ImGui tools B1).  
Score **5 (Linux)** does **not** require Win/mac. Windows stubs exist so the gap is explicit;
they do **not** produce MSVC libs and are **not** required for CI.

| ID | Package | `tools/build_windows.ps1` | Linux status |
|----|---------|---------------------------|--------------|
| **U1** | **ori-stb** | deferred stub (echo + exit 0) | **5 (Linux)** 0.2.0 |
| **U2** | **ori-noise** | deferred stub | **5 (Linux)** 0.2.0 |
| **U3** | **ori-miniz** | deferred stub | **5 (Linux)** 0.2.0 |
| **U4** | **ori-lz4** | deferred stub | **5 (Linux)** 0.2.0 |
| **U5** | **ori-nfd** | deferred stub | **5 (Linux)** 0.2.0 |
| **U6** | **ori-implot** | deferred stub | **5 (Linux)** 0.2.0 |
| **U7** | **ori-imnodes** | deferred stub | **5 (Linux)** 0.2.0 |
| **U8** | **ori-imguizmo** | deferred stub | **5 (Linux)** **0.3.0** (tools B1) |
| **U9** | **ori-tracy** | deferred stub | **5 (Linux)** 0.2.0 |
| **U10** | **ori-enkiTS** | deferred stub | **5 (Linux)** 0.2.0 |
| **U11** | **ori-cgltf** | deferred stub | **5 (Linux)** 0.2.0 |
| **U12** | **ori-fast-obj** | deferred stub | **5 (Linux)** 0.2.0 |
| **U13** | **ori-physfs** | deferred stub | **5 (Linux)** 0.2.0 |
| **U14** | **ori-clay** | deferred stub | **5 (Linux)** 0.2.0 |
| **U15** | **ori-recast** | deferred stub | **5 (Linux)** 0.2.0 |

Each package README has a short **Phase OS** section pointing here.

> Historical note: M1–M6 “medium” labels (cgltf, fast-obj, physfs, clay, lz4, recast)
> were the 0.1.0 ports wave; those six are now part of U1–U15 at **0.2.0**.

## ImGui tools residual packages — Linux **complete**; Phase OS **deferred**

Plan [`pr-plan-imgui-tools-maturity-5.md`](pr-plan-imgui-tools-maturity-5.md) (`a68f7529`)
**PRs 1–15 complete**. New packages ship **5 (Linux)** with README **Phase OS** +
`tools/build_windows.ps1` deferred stub (echo + exit 0). Deepens of existing packages
noted below. **No multi-OS CI green required.**

| ID | Package | Ver. | `tools/build_windows.ps1` | Notes |
|----|---------|------|---------------------------|--------|
| **A1** | **ori-imguidialog** | **0.1.0** | deferred stub | pure Ori open/save |
| **B1** | **ori-imguizmo** | **0.3.0** | deferred stub | TRS + CurveEdit/Gradient/Zoom |
| **B2–B3/C4** | **ori-imgui** | **0.5.1** | MSVC host stub (core table) | timeline + multi-ctx/image + `test_harness` |
| **B4** | **ori-imgui-extras** | **0.1.0** | deferred stub | notify/search/hotkey/palette/metrics |
| **B5** | **ori-imgui-texinspect** | **0.1.0** | deferred stub | texture inspect |
| **C1** | **ori-imgui-textedit** | **0.1.0** | deferred stub | buffer + highlight stub |
| **C2** | **ori-imgui-widgets** | **0.1.0** | deferred stub | knobs/toggle/spinner/spectrum |
| **C3** | **ori-imgui-memory** | **0.1.0** | deferred stub | hex memory editor |
| **D1** | **ori-implot3d** | **0.1.0** | deferred stub | ImPlot3D series milli |
| **D2–D3** | **ori-imgui-markdown** | **0.1.0** | deferred stub | markdown + IME Linux no-ops; real IMM32 = Win Phase OS |

Each package README has a short **Phase OS** section pointing here.

### Umbrella

```powershell
cd C:\path\to\ori-game
.\tools\smoke_eco_windows.ps1 -Stub   # recommended first run (core stack)
# later: without -Stub if real raylib.lib is staged
# U-ports / ImGui tools: run each package tools/build_windows.ps1 only when implementing real MSVC
```

## Checklist (execute on Windows — **last**, when a host is available)

| # | Package | Build | Smoke | Notes |
|---|---------|-------|-------|-------|
| 1 | ori-game | [ ] | [ ] | Start with `-Stub` |
| 2 | ori-box2d | [ ] | [ ] | needs cmake |
| 3 | ori-sqlite | [ ] | [ ] | amalgamation under vendor/ |
| 4 | ori-rres | [ ] | [ ] | |
| 5 | ori-jolt | [ ] | [ ] | stub ABI |
| 6 | ori-imgui **0.5.1** | [ ] | [ ] | host stub until GLFW full |
| 7 | ori-raygui | [ ] | [ ] | real raylib for GUI demos |
| 8+ | U1–U15 | [ ] | [ ] | replace deferred stubs with real MSVC builds |
| 9+ | ImGui tools residual | [ ] | [ ] | deferred stubs today; IME real path on markdown |

## ori-game details

- **Stub:** `tools/setup_raylib_windows.ps1 -Stub` compiles `tools/raylib_stub.c` → `ori_raylib_shim.lib` + tiny `raylib.lib`.
- **Real:** place `raylib.h` + large `raylib.lib`, re-run without `-Stub` to compile `native/ori_raylib_shim.c`.

## Acceptance for “5 (Linux+Win)”

- [ ] Core + selected U-port + ImGui tools smokes green on Windows MSVC (stub OK for graphics packages)
- [ ] Matrix Table A: maturity **5 (Linux+Win)** where claimed
- [ ] Optional: integration demos staged for Win triples
- [ ] Optional: real IMM32 IME path for `ori-imgui-markdown` on Windows

**Not a gate for maturity-5 or ImGui tools plan closeout** — both plans complete at **5 (Linux)**.

## Linux reference (already green)

```bash
export ORI_BIN=ori ORI_USE_SYSTEM_LINKER=1
~/Documentos/Projetos/game-engine-full/ori-game/tools/smoke_eco_linux.sh
```
