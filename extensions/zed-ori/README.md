# Ori — Zed extension

Language support for **Ori** (`.orl`) in [Zed](https://zed.dev).

Portuguese: [README.pt-BR.md](README.pt-BR.md).

- Language config (`.orl`, `--` and `--| |--` comments, all Ori delimiters)
- **LSP** via `ori-lsp` on `PATH`

Extension artifact is versioned separately from the compiler workspace
(`0.3.8-dev`, latest release `v0.3.8`).

## Install

### From GitHub Release (recommended)

1. Install Ori so `ori-lsp` is on your `PATH`  
   ([docs/install.md](../../docs/install.md)).
2. Download **`ori-zed-0.3.5.zip`** from  
   [GitHub Releases](https://github.com/raillen/ori-lang/releases) and extract it.
3. In Zed: command palette → **zed: install dev extension** → select the extracted folder  
   (`ori-zed-0.3.5/`).

Not published to the Zed extension store yet — **dev extension** / release zip only.

### From this monorepo

```text
extensions/zed-ori
```

Zed: **zed: install dev extension** → select that directory.

Or symlink (Linux):

```bash
mkdir -p ~/.local/share/zed/extensions/installed
ln -sfn /path/to/ori-lang/extensions/zed-ori ~/.local/share/zed/extensions/installed/ori
```

## Prerequisites

```bash
# Build language tools (if not using a release package)
cd compiler
cargo build -p ori-lsp -p ori-driver
export PATH="$PWD/target/debug:$PATH"
```

## Settings

Optional: force stdlib if auto-detect fails (extension sets `ORI_STDLIB_ROOT` when it finds `stdlib/` in the worktree).

For structured `@cfg`, start Zed with the same environment used by the CLI:
`ORI_TARGET_TRIPLE`, `ORI_EXECUTION_PROFILE`, `ORI_FEATURES`, and
`ORI_NO_DEFAULT_FEATURES`. The current Zed extension API does not expose a
dedicated Ori settings form, so the language server inherits these values from
the editor process.

## Features / limits

| Feature | Status |
|---------|--------|
| Open `.orl` as language Ori | yes |
| `ori-lsp` diagnostics / hover / complete | yes (if on PATH) |
| Cooperative DAP debugger | yes — registered `ori-dap` adapter launches `ori debug --dap` for the active `.orl` file |
| Tree-sitter syntax colors | **not yet** |
| Zed extension store | **not yet** (GitHub zip + dev install) |

## Package for release

```bash
sh tools/package_editor_extensions.sh --force
# → compiler/target/dist/ori-zed-<ver>.zip
```
