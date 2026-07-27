# Windows package & install

Scripts to install a **complete Ori** release on Windows 10/11 and add it to `PATH`.

| Script | Role |
|--------|------|
| **`get.ps1`** | **Scoop-style bootstrap** — `irm … \| iex` (recommended one-liner) |
| `Install-Ori.ps1` | Full installer (zip / GitHub / extracted package + PATH) |
| `Uninstall-Ori.ps1` | Remove install dir + `PATH` entry |
| `install.cmd` / `uninstall.cmd` | Double-click wrappers |

## One-liner (like Scoop)

```powershell
# Optional once per machine (allows local scripts; irm|iex still runs in memory):
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser

# Install latest Windows package + User PATH
irm https://raw.githubusercontent.com/raillen/ori-lang/master/tools/windows/get.ps1 | iex
```

Pin a version or force reinstall via **environment variables** (work with `| iex`):

```powershell
$env:ORI_VERSION = "0.3.5"
$env:ORI_FORCE = "1"
irm https://raw.githubusercontent.com/raillen/ori-lang/master/tools/windows/get.ps1 | iex
```

| Variable | Meaning |
|----------|---------|
| `ORI_VERSION` | e.g. `0.3.5` or `v0.3.5` (default: latest GitHub release) |
| `ORI_INSTALL_DIR` | Custom install folder |
| `ORI_FORCE` | `1` overwrite existing install |
| `ORI_SYSTEM` | `1` → Program Files + Machine PATH (admin) |
| `ORI_SKIP_DOCTOR` | `1` skip `ori doctor` |

With parameters (download the script first, or use a scriptblock):

```powershell
irm https://raw.githubusercontent.com/raillen/ori-lang/master/tools/windows/get.ps1 -OutFile get-ori.ps1
.\get-ori.ps1 -Version 0.3.5 -Force

# or without a file:
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/raillen/ori-lang/master/tools/windows/get.ps1))) -Version 0.3.5 -Force
```

`get.ps1` downloads `Install-Ori.ps1` from the same branch and runs it (full PATH logic).  
If raw.githubusercontent is blocked, it falls back to a minimal zip+PATH install.

## What gets installed

```text
%LOCALAPPDATA%\Programs\Ori\    (default, per-user)
  ori.exe
  ori-lsp.exe
  stdlib\
  runtime\x86_64-pc-windows-msvc\
  install.ps1 / uninstall.ps1
  ori-install.json
```

- **User install (default):** no admin; User `PATH`.
- **System install:** `-System` → `%ProgramFiles%\Ori` + Machine `PATH` (admin).

Stdlib and runtime resolve next to `ori.exe` (no extra env vars required).

## End-user flows

### A — From the release zip (recommended)

1. Download `ori-vX.Y.Z-x86_64-pc-windows-msvc.zip` from
   [GitHub Releases](https://github.com/raillen/ori-lang/releases).
2. Extract anywhere.
3. Run **`install.cmd`** (or):

```powershell
pwsh -ExecutionPolicy Bypass -File .\install.ps1
```

4. Open a **new** terminal:

```powershell
ori --version
ori doctor
```

### B — One-liner from GitHub (when the Windows asset exists)

```powershell
pwsh -ExecutionPolicy Bypass -File tools/windows/Install-Ori.ps1 -Version 0.3.5
# or latest:
pwsh -ExecutionPolicy Bypass -File tools/windows/Install-Ori.ps1
```

### C — From a local zip built on this machine

```powershell
pwsh -File tools/package_native_release.ps1 -Force
pwsh -File tools/windows/Install-Ori.ps1 `
  -ZipPath compiler\target\dist\ori-<ver>-x86_64-pc-windows-msvc.zip `
  -Force
```

## Uninstall

```powershell
pwsh -File "$env:LOCALAPPDATA\Programs\Ori\uninstall.ps1"
# or from this repo:
pwsh -File tools/windows/Uninstall-Ori.ps1
```

## AOT prerequisite

`ori run` (JIT) needs only the packaged `ori_runtime.dll`.  
`ori compile` / `ori test` need **MSVC** `link.exe`:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

Select workload **Desktop development with C++**.

## Maintainer: build the Windows zip

On a Windows machine (or GHA `windows-latest`):

```powershell
pwsh -File tools/ci_package_windows.ps1 -Tag v0.3.5
# → compiler\target\dist\ori-v0.3.5-x86_64-pc-windows-msvc.zip
```

The package scripts copy these installers into the package root automatically
(`smoke_native_release.ps1` on Windows).

## PATH details

| Scope | When | Registry / store |
|-------|------|------------------|
| User | default | `[Environment]::SetEnvironmentVariable(..., 'User')` |
| Machine | `-System` | requires elevation |

The installer also prepends the install dir to the **current process** `PATH`
so `ori` works immediately in that session.
