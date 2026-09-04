# Ori — VS Code / Cursor extension

Language support for **Ori** (`.orl` and `.oridoc`): LSP, grammar, snippets, doctor.

**Surface:** S3/0.4. The compiler
workspace is `0.3.8-dev` (latest release `v0.3.8`); extension artifacts are
versioned separately until rebuilt.

## Install

### From GitHub Release (recommended)

1. Install Ori so `ori` / `ori-lsp` are on your `PATH`  
   ([docs/install.md](../../docs/install.md) — Windows one-liner: `irm …/get.ps1 | iex`).
2. Download **`ori-vscode-orl-0.3.5.vsix`** from  
   [GitHub Releases](https://github.com/raillen/ori-lang/releases).
3. Install:

```bash
code --install-extension ori-vscode-orl-0.3.5.vsix
# Cursor:
cursor --install-extension ori-vscode-orl-0.3.5.vsix
```

Or: VS Code → **Extensions: Install from VSIX…**

Not published to the VS Marketplace yet (local / GitHub release only).

### From this monorepo

```bash
./tools/install_vscode_extension.sh
# or:
cd extensions/vscode-orl && npm install && npm run package:vsix && npm run install:local
```

## Features

- **LSP** via `ori-lsp`: diagnostics, hover, go-to-definition, completion (stdlib), rename, format, semantic tokens, inlay hints
- **Local inference (option B):** inlays for obvious local types
- **Pipe `|>`**
- **Commands:** Check, Run, Test, Debug, Format, Doctor (`ori doctor`), Project Summary
- **Debugger:** the `Ori: Debug Current File` command registers the `ori` debug type and starts `ori debug --dap`; breakpoints, continue/step, call stack, scalar locals, struct field paths, list metadata, async frames, and closure captures appear in the VS Code debug view
- Current-surface TextMate grammar and snippets for `newtype`, compact `apply … use …`, match expressions, or-patterns, conditional bindings, and struct destructuring
- `.oridoc` file detection and highlighting for namespaces, documentation blocks, sections, and inline code
- Interpolated expression highlighting inside single-line and triple-quoted f-strings

## Settings

| Setting | Env | Description |
|---------|-----|-------------|
| `ori.lsp.path` | — | Path to `ori-lsp` |
| `ori.compiler.path` | — | Path to `ori` CLI |
| `ori.stdlib.root` | `ORI_STDLIB_ROOT` | Stdlib directory |
| `ori.cfg.target` | `ORI_TARGET_TRIPLE` | Target facts used by `@cfg` |
| `ori.cfg.executionProfile` | `ORI_EXECUTION_PROFILE` | `standalone` or `embedded` |
| `ori.cfg.features` | `ORI_FEATURES` | Manifest-declared enabled features |
| `ori.cfg.noDefaultFeatures` | `ORI_NO_DEFAULT_FEATURES=1` | Disable manifest defaults |
| `ori.runtime.lib` | `ORI_RUNTIME_LIB` | Native staticlib |
| `ori.runtime.cdylib` | `ORI_RUNTIME_CDYLIB` | JIT cdylib |
| `ori.useJit` | `ORI_USE_JIT=1` | Force JIT for extension terminals (default true) |
| `ori.useAot` | `ORI_USE_AOT=1` | Force AOT for extension terminals |

Changing any `ori.cfg.*` setting restarts the language server automatically so
diagnostics, completion, CLI terminals, and debugging keep one active program.

Binary discovery (when paths empty): `PATH`, then monorepo  
`compiler/target/{debug,release}/`, then root `target/{debug,release}/`.

To create a workspace launch configuration, use **Run and Debug → create a
`launch.json` file → Ori**. The generated configuration is equivalent to:

```json
{
  "type": "ori",
  "request": "launch",
  "name": "Debug Ori file",
  "program": "${file}"
}
```

The adapter currently exposes synchronous and async stack frames, scalar locals,
structured `struct`/`optional`/`result`/enum and collection paths, bounded list
length/capacity and indexed children, closure captures, and bounded previews
for managed strings and bytes (bytes are shown in hexadecimal). Static or
foreign buffers require an exact registered length. The `evaluate` request
supports side-effect-free scalar arithmetic, comparisons, boolean logic, and
strings from the latest stopped snapshot; it never executes target code.

## Development

```bash
cd compiler && cargo build -p ori-lsp -p ori-driver
cd ../extensions/vscode-orl
npm install
npm run compile
```

F5 in VS Code → Extension Development Host.

Repo smoke: `./tools/smoke_vscode_extension.sh`

## Package for release

```bash
sh tools/package_editor_extensions.sh --force
# → compiler/target/dist/ori-vscode-orl-<ver>.vsix
```
