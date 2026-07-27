# Packages — **not stored in this monorepo**

**Policy (2026-07-16):** Ori packages (libraries, bindings, Lantern web libs) are
**not** developed or kept under `ori-lang/packages/`.

| What | Where |
|------|--------|
| **All official packages** | [`ori-official-packages`](https://github.com/raillen/ori-official-packages) → `packages/` |
| **Policy** | [PACKAGE-HOME.md](https://github.com/raillen/ori-official-packages/blob/main/docs/PACKAGE-HOME.md) |
| **Lantern (web) day-to-day** | [`ori-web-framework`](https://github.com/raillen/ori-web-framework) |
| **Registry UI** | [`ori-lamp`](https://github.com/raillen/ori-lamp) |
| **Game/FFI lab** | `game-engine-full` (local) — promote to official-packages before release |

## Why this folder exists

Historical web demos and package trees lived here. They were **removed** to
enforce a single home and stop triple copies (`ori-lang` × `web-framework` ×
`official-packages`).

If you need a package:

```bash
# registry (when ORI_REGISTRY points at OriLamp)
ori install web@0.1.0
ori install sqlite@0.3.0

# path dep during development
# ../ori-official-packages/packages/ori-sqlite
# or ../ori-web-framework/packages/ori-web  (Lantern only)
```

## Forbidden

- Adding `ori-web/`, `ori-templates/`, `ori-raylib/`, demos as package trees here
- Symlinking half the official monorepo into this directory

Language FREEZE-1 and compiler work stay under `compiler/`, `stdlib/`, `docs/spec/`.
Web feature freeze docs live in **ori-web-framework** (`FREEZE-WEB.md`).
