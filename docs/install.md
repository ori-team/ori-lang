# Installing Ori

> **Audience:** end users who want to write Ori programs **without** cloning this
> repository and **without** a Rust toolchain.  
> **Portuguese:** [install.pt-BR.md](install.pt-BR.md)  
> **Surface:** S3 · package **v0.3.4** · M1 (Rust-free install path) complete · FREEZE-1 on 0.3.x

## System prerequisites

Ori uses the **OS native linker** for AOT (`ori compile`, `ori test`).  
For JIT (`ori run`), no linker is required — only the packaged runtime next to
the `ori` binary (`runtime/<triple>/`).

### Windows (10/11)

**Requirement:** Visual Studio Build Tools or Visual Studio Community with the
**"Desktop development with C++"** workload.

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

Or the installer at [visualstudio.microsoft.com/downloads](https://visualstudio.microsoft.com/downloads/).

**Why:** AOT uses MSVC `link.exe`.

**Not required:** Rust (`rustc`, `cargo`), or `rust-lld` (SystemLinker is the default).

### Linux

**Requirement:** `build-essential` (or `gcc` + `ld` + libc headers).

```bash
# Debian / Ubuntu
sudo apt update && sudo apt install build-essential

# Fedora / RHEL
sudo dnf install gcc gcc-c++ make glibc-devel

# Arch
sudo pacman -S base-devel
```

**Not required:** Rust.

### macOS

**Requirement:** Xcode Command Line Tools.

```bash
xcode-select --install
```

**Not required:** full Xcode, Rust, or `rust-lld`.

---

## Download and install

### Release package (recommended)

> **Shipping policy (2026-07-14):** official **release packages** for **Linux,
> Windows (MSVC), and macOS** (Apple Silicon + Intel) via GitHub Actions
> (`.github/workflows/release.yml`). Assets appear on
> [GitHub Releases](https://github.com/raillen/ori-lang/releases) after a `v*` tag.

1. Download from [GitHub Releases](https://github.com/raillen/ori-lang/releases).
   Example names for **v0.3.5** (version matches the tag):

   | Platform | Asset |
   |----------|--------|
   | Linux x86_64 | `ori-v0.3.5-x86_64-unknown-linux-gnu.tar.gz` |
   | Linux deb | `ori_0.3.5_amd64.deb` |
   | Windows MSVC x86_64 | `ori-v0.3.5-x86_64-pc-windows-msvc.zip` |
   | macOS Apple Silicon | `ori-v0.3.5-aarch64-apple-darwin.tar.gz` |
   | macOS Intel | `ori-v0.3.5-x86_64-apple-darwin.tar.gz` |

#### Option A — Windows one-liner (recommended, Scoop-style)

```powershell
# Optional once: allow local scripts (CurrentUser)
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser

# Install latest Ori + add User PATH
irm https://raw.githubusercontent.com/raillen/ori-lang/master/tools/windows/get.ps1 | iex
```

Pin version / reinstall:

```powershell
$env:ORI_VERSION = "0.3.5"
$env:ORI_FORCE = "1"
irm https://raw.githubusercontent.com/raillen/ori-lang/master/tools/windows/get.ps1 | iex
```

Default install location: `%LOCALAPPDATA%\Programs\Ori`  
(all-users: `$env:ORI_SYSTEM = "1"` as Administrator).

Then open a **new** terminal:

```powershell
ori --version
ori doctor
```

Uninstall:

```powershell
irm https://raw.githubusercontent.com/raillen/ori-lang/master/tools/windows/Uninstall-Ori.ps1 | iex
# or:
pwsh -File "$env:LOCALAPPDATA\Programs\Ori\uninstall.ps1"
```

#### Option A2 — Windows zip (offline / manual)

1. Download `ori-vX.Y.Z-x86_64-pc-windows-msvc.zip` and extract it.
2. Run **`install.cmd`** or `pwsh -ExecutionPolicy Bypass -File .\install.ps1`.

Full details: [`tools/windows/README.md`](../tools/windows/README.md).

#### Option B — tarball / zip (manual PATH)

2. Extract to a directory (e.g. `~/ori` or `C:\ori`).

3. Expected layout:

   | Path | Role |
   |------|------|
   | `ori` / `ori.exe` | CLI |
   | `ori-lsp` / `ori-lsp.exe` | LSP server |
   | `stdlib/` | Layer 2/3 `.orl` modules |
   | `runtime/<triple>/` | staticlib + cdylib + `runtime-link.json` |
   | `install.ps1` / `install.cmd` | Windows only — installer + PATH |

4. Put the directory on your `PATH` (on Windows prefer **Option A**).

#### Option C — `.deb` (Debian / Ubuntu)

```bash
sudo dpkg -i ori_0.3.5_amd64.deb
# installs /usr/lib/ori + /usr/bin/ori + /usr/bin/ori-lsp
# AOT still needs: sudo apt install build-essential
```

### Verify

```bash
ori --version
ori doctor
```

Healthy install: stdlib found, AOT + JIT runtime present, target triple detected,
linker strategy **SystemLinker** (or documented fallback), JIT available for
`ori run`.

---

## First program

`hello.orl` (S3):

```ori
module app.hello

import ori.io = io

main()
    io.println("Hello, Ori!")
end
```

```bash
ori run hello.orl                 # JIT — no linker
ori compile hello.orl --out hello # AOT — needs system linker
./hello
```

Recommended project skeleton:

```bash
ori new my_app
cd my_app
ori run main.orl
```

### Editor extensions (VS Code / Zed)

From the same [GitHub Release](https://github.com/raillen/ori-lang/releases) as the language package:

| Editor | Asset | Install |
|--------|--------|---------|
| VS Code / Cursor | `ori-vscode-orl-0.3.5.vsix` | `code --install-extension ori-vscode-orl-0.3.5.vsix` |
| Zed | `ori-zed-0.3.5.zip` | extract → **zed: install dev extension** |

Requires `ori-lsp` on `PATH` (from the language install above).  
Details: [`extensions/README.md`](../extensions/README.md).

Next: [Language tour](language/tour.md) · [First project](guides/first-project.md) ·
[Examples](../examples/) · Editors: [VS Code](../extensions/vscode-orl/) ·
[Zed](../extensions/zed-ori/).

---

## Updating

Package installs (tar.gz / Windows zip) can update themselves:

```console
$ ori update --check   # report whether a newer release exists
$ ori update           # download, verify (sha256), and swap in place
```

`ori update` refuses system-package installs (use the new `.deb` instead)
and development builds (update via `git pull` + `cargo build`). The archive
checksum comes from the GitHub release manifest; a mismatch aborts before
anything is touched.

---

## Environment overrides

Usually **none** are needed.

| Variable | Purpose |
|----------|---------|
| `ORI_USE_SYSTEM_LINKER=1` | Force OS linker |
| `ORI_SYSTEM_LINKER` | Explicit linker path |
| `ORI_USE_BUNDLED_RUST_LLD=1` | Force bundled `rust-lld` |
| `ORI_USE_RUSTC_DRIVER=1` | Legacy `rustc` driver (not for end users) |
| `ORI_USE_JIT=1` / `ORI_USE_AOT=1` | Force `ori run` mode |
| `ORI_RUNTIME_CDYLIB` / `ORI_RUNTIME_LIB` | Runtime path overrides |
| `ORI_STDLIB_ROOT` | Stdlib path override |
| `ORI_REQUIRE_PACKAGED_RUNTIME=1` | Package-only runtime (smoke/release) |

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `native.link_failed` / linker not found | Install OS linker prereqs; check `link.exe` / `ld` / `xcrun --find ld` |
| Runtime not found | Keep `runtime/` beside `ori` |
| `ori run` works, `ori compile` fails | Install system linker (AOT only) |
| VS Code / LSP | Put `ori-lsp` on `PATH` or set `ori.lsp.path` / `ori.compiler.path` / `ori.stdlib.root` |

---

## Maintainer package smoke

```bash
sh tools/package_native_release.sh --force
sh tools/smoke_no_rust.sh --package-root compiler/target/dist/ori-… --allow-rust-on-path
```

See [AGENTS.md](../AGENTS.md) and [spec/19-abi.md](spec/19-abi.md) (`ori-native-abi-1`).
