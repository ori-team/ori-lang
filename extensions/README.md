# Ori editor extensions

Extension artifacts are compatible with the current S3/0.4 language surface;
the compiler workspace itself is `0.3.8-dev` (latest release `v0.3.8`).
**GitHub Release assets** ship the installers — not the VS Marketplace / Zed store yet.

| Path | Editor | Release asset |
|------|--------|----------------|
| [`vscode-orl/`](vscode-orl/) | VS Code / Cursor | `ori-vscode-orl-0.3.5.vsix` |
| [`zed-ori/`](zed-ori/) | Zed | `ori-zed-0.3.5.zip` (dev extension) |

## Install from GitHub Release

Assets on [ori-lang releases](https://github.com/raillen/ori-lang/releases) (the
currently documented extension assets are from **v0.3.5**).

### VS Code / Cursor

```bash
# Download ori-vscode-orl-0.3.5.vsix from the release, then:
code --install-extension ori-vscode-orl-0.3.5.vsix
# or Cursor:
cursor --install-extension ori-vscode-orl-0.3.5.vsix
```

Requires `ori-lsp` on `PATH` (install Ori first — Windows: `irm …/get.ps1 | iex`).
The VS Code extension also registers the `ori` debugger type and starts
`ori debug --dap` for breakpoints, continue/step, stack frames, and scalar
locals.

### Zed

1. Download and extract `ori-zed-0.3.5.zip`.
2. Zed command palette → **zed: install dev extension** → select the extracted folder.
3. Ensure `ori-lsp` is on `PATH`.

The Zed extension exposes language-server integration and DAP debugger integration
(`ori-dap`), launching `ori debug --dap` for breakpoints, step debugging, and inspection.

## Build locally

```bash
# Language tools
cd compiler && cargo build -p ori-lsp -p ori-driver

# Both release artifacts → compiler/target/dist/
sh tools/package_editor_extensions.sh --force
```
