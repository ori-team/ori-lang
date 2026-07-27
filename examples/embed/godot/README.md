# P4 sketch — Godot 4.x + Ori cdylib (GDExtension)

This directory documents the **P4** acceptance path from
[`PLANO-CDYLIB-EMBED.md`](../../../docs/planning/PLANO-CDYLIB-EMBED.md).

Full GDExtension C++ registration is **host-side** (Godot 4.x headers). Ori
supplies the **logic library** built with:

```bash
ori compile --lib game_logic.orl -o libori_game_logic.so
```

## Host flow (Compatibility renderer)

1. Build Ori lib (`@c_export` methods for your game API).
2. Write a thin GDExtension C/C++ shim that:
   - `dlopen`s `libori_game_logic.so` (and resolves `libori_runtime.so`)
   - calls `ori_rt_init()` + `__ori_module_init()`
   - binds exported Ori functions to Godot methods / callables
3. Load the extension in a Godot 4.x project (Compatibility, low-end GPU OK).

## Performance gate (plan)

At 60fps, measure host→Ori call cost on a representative method. Target
`≤ 2µs/call` for the example module (ties to language issue #1).

Current P1 harness on Linux measures ~tens of ns for pure `int` add (see
`tools/qa/embed_smoke.sh`) — well under budget for scalar exports.

## Status

| Piece | Status |
|-------|--------|
| Ori `--lib` + `@c_export` | done (P1) |
| Smoke harness C | done |
| Full Godot GDExtension project in-tree | **stub** — implement when Godot SDK is wired in CI |

See also: `examples/embed/README.md` for the language/host contract.
