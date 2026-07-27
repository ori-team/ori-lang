#Requires -Version 5.1
<#
.SYNOPSIS
  Install the Ori language package on Windows and add it to PATH.

.DESCRIPTION
  Installs a complete Ori release layout (ori.exe, ori-lsp.exe, stdlib/, runtime/)
  into a fixed directory and permanently adds that directory to the User PATH
  (or Machine PATH with -System).

  Sources (first match wins):
    1. -PackageRoot  — already extracted package directory
    2. -ZipPath      — local release zip
    3. Script parent if it already contains ori.exe (install from extracted zip)
    4. -Version / latest GitHub release asset for x86_64-pc-windows-msvc

.PARAMETER InstallDir
  Destination directory. Default: %LOCALAPPDATA%\Programs\Ori

.PARAMETER System
  Install for all users into Program Files and update Machine PATH (requires admin).

.PARAMETER NoPath
  Copy files only; do not modify PATH.

.PARAMETER Force
  Overwrite an existing InstallDir.

.EXAMPLE
  # From extracted release folder:
  .\install.ps1

.EXAMPLE
  # From repo after packaging:
  pwsh -File tools/windows/Install-Ori.ps1 -ZipPath compiler\target\dist\ori-v0.3.5-x86_64-pc-windows-msvc.zip

.EXAMPLE
  # Download latest Windows package from GitHub and install:
  pwsh -File tools/windows/Install-Ori.ps1 -Version 0.3.5
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [string]$InstallDir = "",
    [string]$PackageRoot = "",
    [string]$ZipPath = "",
    [string]$Version = "",
    [string]$Repo = "raillen/ori-lang",
    [switch]$System,
    [switch]$NoPath,
    [switch]$Force,
    [switch]$SkipDoctor
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

$TargetTriple = "x86_64-pc-windows-msvc"
$GithubApi = "https://api.github.com/repos/$Repo/releases"

function Write-Info([string]$Message) { Write-Host "[ori-install] $Message" }
function Write-Warn([string]$Message) { Write-Warning "[ori-install] $Message" }

function Test-IsAdministrator {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($id)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-DefaultInstallDir {
    if ($System) {
        return (Join-Path ${env:ProgramFiles} "Ori")
    }
    return (Join-Path $env:LOCALAPPDATA "Programs\Ori")
}

function Add-PathEntry {
    param(
        [Parameter(Mandatory = $true)][string]$Directory,
        [ValidateSet("User", "Machine")][string]$Scope = "User"
    )
    $normalized = [System.IO.Path]::GetFullPath($Directory).TrimEnd('\')
    $current = [Environment]::GetEnvironmentVariable("Path", $Scope)
    if ($null -eq $current) { $current = "" }

    $parts = @()
    foreach ($p in ($current -split ';' | ForEach-Object { $_.Trim() })) {
        if ([string]::IsNullOrWhiteSpace($p)) { continue }
        try {
            $full = [System.IO.Path]::GetFullPath($p).TrimEnd('\')
        } catch {
            $full = $p.TrimEnd('\')
        }
        if ($full -ieq $normalized) {
            Write-Info "PATH ($Scope) already contains $normalized"
            # Still refresh process PATH.
            if (($env:Path -split ';' | ForEach-Object { $_.TrimEnd('\') }) -notcontains $normalized) {
                $env:Path = "$normalized;$env:Path"
            }
            return
        }
        $parts += $p
    }

    $newPath = if ($parts.Count -eq 0) { $normalized } else { ($parts + $normalized) -join ';' }
    if ($PSCmdlet.ShouldProcess("PATH ($Scope)", "Add $normalized")) {
        [Environment]::SetEnvironmentVariable("Path", $newPath, $Scope)
        Write-Info "Added to $Scope PATH: $normalized"
    }
    # Current session
    $env:Path = "$normalized;$env:Path"
}

function Remove-PathEntry {
    param(
        [Parameter(Mandatory = $true)][string]$Directory,
        [ValidateSet("User", "Machine")][string]$Scope = "User"
    )
    $normalized = [System.IO.Path]::GetFullPath($Directory).TrimEnd('\')
    $current = [Environment]::GetEnvironmentVariable("Path", $Scope)
    if ([string]::IsNullOrWhiteSpace($current)) { return }

    $kept = New-Object System.Collections.Generic.List[string]
    $removed = $false
    foreach ($p in ($current -split ';')) {
        $t = $p.Trim()
        if ([string]::IsNullOrWhiteSpace($t)) { continue }
        try {
            $full = [System.IO.Path]::GetFullPath($t).TrimEnd('\')
        } catch {
            $full = $t.TrimEnd('\')
        }
        if ($full -ieq $normalized) {
            $removed = $true
            continue
        }
        $kept.Add($t) | Out-Null
    }
    if ($removed) {
        [Environment]::SetEnvironmentVariable("Path", ($kept -join ';'), $Scope)
        Write-Info "Removed from $Scope PATH: $normalized"
    }
}

function Assert-PackageLayout([string]$Root) {
    $ori = Join-Path $Root "ori.exe"
    $lsp = Join-Path $Root "ori-lsp.exe"
    $stdlib = Join-Path $Root "stdlib"
    $runtime = Join-Path $Root "runtime"
    if (-not (Test-Path -LiteralPath $ori -PathType Leaf)) {
        throw "Package missing ori.exe under $Root"
    }
    if (-not (Test-Path -LiteralPath $lsp -PathType Leaf)) {
        throw "Package missing ori-lsp.exe under $Root"
    }
    if (-not (Test-Path -LiteralPath $stdlib -PathType Container)) {
        throw "Package missing stdlib/ under $Root"
    }
    if (-not (Test-Path -LiteralPath $runtime -PathType Container)) {
        throw "Package missing runtime/ under $Root"
    }
    # Prefer full native triple layout when present.
    $tripleDir = Join-Path $runtime $TargetTriple
    if (Test-Path -LiteralPath $tripleDir -PathType Container) {
        $dll = Join-Path $tripleDir "ori_runtime.dll"
        if (-not (Test-Path -LiteralPath $dll -PathType Leaf)) {
            Write-Warn "runtime/$TargetTriple/ori_runtime.dll not found — JIT may be unavailable."
        }
    } else {
        Write-Warn "runtime/$TargetTriple not found — layout may still work if runtime is flat."
    }
}

function Find-ExtractedPackageRoot([string]$ExtractRoot) {
    # Zip may contain a single top-level folder or flat files.
    $direct = Join-Path $ExtractRoot "ori.exe"
    if (Test-Path -LiteralPath $direct -PathType Leaf) {
        return (Resolve-Path -LiteralPath $ExtractRoot).Path
    }
    $children = Get-ChildItem -LiteralPath $ExtractRoot -Directory -ErrorAction SilentlyContinue
    foreach ($dir in $children) {
        if (Test-Path -LiteralPath (Join-Path $dir.FullName "ori.exe") -PathType Leaf) {
            return $dir.FullName
        }
    }
    throw "Could not find ori.exe under extracted content at $ExtractRoot"
}

function Expand-ZipToTemp([string]$Zip) {
    if (-not (Test-Path -LiteralPath $Zip -PathType Leaf)) {
        throw "Zip not found: $Zip"
    }
    $temp = Join-Path ([System.IO.Path]::GetTempPath()) ("ori-install-" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $temp | Out-Null
    Write-Info "Extracting $Zip → $temp"
    Expand-Archive -LiteralPath $Zip -DestinationPath $temp -Force
    return (Find-ExtractedPackageRoot $temp), $temp
}

function Get-ReleaseAssetUrl {
    param(
        [string]$WantVersion,
        [string]$Repository
    )
    $headers = @{
        "User-Agent" = "ori-windows-install"
        "Accept"     = "application/vnd.github+json"
    }
    if ($env:GITHUB_TOKEN) {
        $headers["Authorization"] = "Bearer $env:GITHUB_TOKEN"
    }

    if ([string]::IsNullOrWhiteSpace($WantVersion)) {
        $release = Invoke-RestMethod -Uri "$GithubApi/latest" -Headers $headers
    } else {
        $tag = if ($WantVersion.StartsWith("v")) { $WantVersion } else { "v$WantVersion" }
        $release = Invoke-RestMethod -Uri "$GithubApi/tags/$tag" -Headers $headers
    }

    $asset = $release.assets | Where-Object {
        $_.name -match 'x86_64-pc-windows-msvc\.zip$' -or
        $_.name -match 'windows-msvc\.zip$'
    } | Select-Object -First 1

    if (-not $asset) {
        $names = @($release.assets | ForEach-Object { $_.name }) -join ", "
        throw "No Windows MSVC zip on release $($release.tag_name). Assets: $names"
    }
    Write-Info "Using release asset $($asset.name) ($($release.tag_name))"
    return $asset.browser_download_url, $asset.name
}

function Download-File([string]$Url, [string]$OutPath) {
    Write-Info "Downloading $Url"
    $dir = Split-Path -Parent $OutPath
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    }
    # Prefer curl.exe when available (better large-file progress); else IRM.
    $curl = Get-Command curl.exe -ErrorAction SilentlyContinue
    if ($curl) {
        & curl.exe -fsSL -L -o $OutPath $Url
        if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
            throw "curl download failed with exit code $LASTEXITCODE"
        }
    } else {
        Invoke-WebRequest -Uri $Url -OutFile $OutPath -UseBasicParsing
    }
    if (-not (Test-Path -LiteralPath $OutPath -PathType Leaf)) {
        throw "Download failed: $OutPath missing"
    }
}

function Copy-Package([string]$SourceRoot, [string]$DestRoot) {
    Write-Info "Installing from $SourceRoot → $DestRoot"
    if (Test-Path -LiteralPath $DestRoot) {
        if (-not $Force) {
            throw "Install directory already exists: $DestRoot (pass -Force to replace)"
        }
        if ($PSCmdlet.ShouldProcess($DestRoot, "Remove existing install")) {
            Remove-Item -LiteralPath $DestRoot -Recurse -Force
        }
    }
    New-Item -ItemType Directory -Force -Path $DestRoot | Out-Null

    # Copy package contents (not the outer wrapper folder name).
    Get-ChildItem -LiteralPath $SourceRoot -Force | ForEach-Object {
        $dest = Join-Path $DestRoot $_.Name
        Copy-Item -LiteralPath $_.FullName -Destination $dest -Recurse -Force
    }

    # Ensure uninstall helpers ship with the install.
    $toolsWindows = $PSScriptRoot
    foreach ($name in @("Install-Ori.ps1", "Uninstall-Ori.ps1", "install.cmd", "uninstall.cmd")) {
        $src = Join-Path $toolsWindows $name
        if (Test-Path -LiteralPath $src -PathType Leaf) {
            Copy-Item -LiteralPath $src -Destination (Join-Path $DestRoot $name) -Force
        }
    }
    # Friendly short names at install root.
    $installPs1 = Join-Path $DestRoot "Install-Ori.ps1"
    if (Test-Path -LiteralPath $installPs1) {
        Copy-Item -LiteralPath $installPs1 -Destination (Join-Path $DestRoot "install.ps1") -Force
    }
    $uninstallPs1 = Join-Path $DestRoot "Uninstall-Ori.ps1"
    if (Test-Path -LiteralPath $uninstallPs1) {
        Copy-Item -LiteralPath $uninstallPs1 -Destination (Join-Path $DestRoot "uninstall.ps1") -Force
    }
}

function Write-InstallManifest([string]$DestRoot, [string]$SourceLabel) {
    $manifest = [ordered]@{
        installedAt   = (Get-Date).ToString("o")
        installDir    = $DestRoot
        pathScope     = $(if ($System) { "Machine" } else { "User" })
        source        = $SourceLabel
        targetTriple  = $TargetTriple
        installer     = $PSCommandPath
    }
    $json = $manifest | ConvertTo-Json -Depth 4
    Set-Content -LiteralPath (Join-Path $DestRoot "ori-install.json") -Value $json -Encoding UTF8
}

# --- main ---

if ($System -and -not (Test-IsAdministrator)) {
    throw " -System requires an elevated PowerShell (Run as administrator)."
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Get-DefaultInstallDir
}
$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$pathScope = if ($System) { "Machine" } else { "User" }

$cleanupTemp = $null
$sourceRoot = $null
$sourceLabel = ""

try {
    if (-not [string]::IsNullOrWhiteSpace($PackageRoot)) {
        $sourceRoot = (Resolve-Path -LiteralPath $PackageRoot).Path
        $sourceLabel = "PackageRoot:$sourceRoot"
    }
    elseif (-not [string]::IsNullOrWhiteSpace($ZipPath)) {
        $pair = Expand-ZipToTemp -Zip (Resolve-Path -LiteralPath $ZipPath).Path
        $sourceRoot = $pair[0]
        $cleanupTemp = $pair[1]
        $sourceLabel = "ZipPath:$ZipPath"
    }
    else {
        # Running from inside an extracted package (install.ps1 next to ori.exe)
        $here = $PSScriptRoot
        if (Test-Path -LiteralPath (Join-Path $here "ori.exe") -PathType Leaf) {
            $sourceRoot = $here
            $sourceLabel = "ExtractedPackage:$here"
        }
        else {
            # Download from GitHub
            $url, $assetName = Get-ReleaseAssetUrl -WantVersion $Version -Repository $Repo
            $zipOut = Join-Path ([System.IO.Path]::GetTempPath()) $assetName
            Download-File -Url $url -OutPath $zipOut
            $pair = Expand-ZipToTemp -Zip $zipOut
            $sourceRoot = $pair[0]
            $cleanupTemp = $pair[1]
            $sourceLabel = "GitHub:$url"
            Remove-Item -LiteralPath $zipOut -Force -ErrorAction SilentlyContinue
        }
    }

    Assert-PackageLayout -Root $sourceRoot

    # Avoid recursive wipe if installing from the same folder.
    $sameDir = ([System.IO.Path]::GetFullPath($sourceRoot).TrimEnd('\') -ieq $InstallDir.TrimEnd('\'))
    if ($sameDir) {
        Write-Info "Package already at install location $InstallDir — updating PATH only."
    } else {
        Copy-Package -SourceRoot $sourceRoot -DestRoot $InstallDir
    }

    Assert-PackageLayout -Root $InstallDir
    Write-InstallManifest -DestRoot $InstallDir -SourceLabel $sourceLabel

    if (-not $NoPath) {
        Add-PathEntry -Directory $InstallDir -Scope $pathScope
    } else {
        Write-Info "Skipped PATH update (-NoPath)."
    }

    $oriExe = Join-Path $InstallDir "ori.exe"
    Write-Info "Verifying $oriExe"
    $ver = & $oriExe --version 2>&1
    Write-Host ($ver | Out-String).Trim()

    if (-not $SkipDoctor) {
        Write-Info "Running ori doctor"
        $prev = $env:ORI_REQUIRE_PACKAGED_RUNTIME
        $env:ORI_REQUIRE_PACKAGED_RUNTIME = "1"
        try {
            & $oriExe doctor
        } finally {
            if ($null -eq $prev) {
                Remove-Item Env:\ORI_REQUIRE_PACKAGED_RUNTIME -ErrorAction SilentlyContinue
            } else {
                $env:ORI_REQUIRE_PACKAGED_RUNTIME = $prev
            }
        }
    }

    Write-Host ""
    Write-Host "Ori installed successfully." -ForegroundColor Green
    Write-Host "  Location : $InstallDir"
    Write-Host "  PATH     : $pathScope (open a new terminal if 'ori' is not found)"
    Write-Host ""
    Write-Host "Try:"
    Write-Host "  ori --version"
    Write-Host "  ori doctor"
    Write-Host "  ori new hello && cd hello && ori run main.orl"
    Write-Host ""
    Write-Host "AOT (ori compile) needs Visual Studio Build Tools with C++ workload."
    Write-Host "  winget install Microsoft.VisualStudio.2022.BuildTools"
    Write-Host ""
    Write-Host "Uninstall:"
    Write-Host "  pwsh -File `"$(Join-Path $InstallDir 'uninstall.ps1')`""
}
finally {
    if ($cleanupTemp -and (Test-Path -LiteralPath $cleanupTemp)) {
        Remove-Item -LiteralPath $cleanupTemp -Recurse -Force -ErrorAction SilentlyContinue
    }
}
