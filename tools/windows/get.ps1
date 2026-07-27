#Requires -Version 5.1
<#
.SYNOPSIS
  Scoop-style bootstrap: install Ori on Windows with one line (irm | iex).

.DESCRIPTION
  Downloads the official Windows MSVC release zip from GitHub, installs it under
  %LOCALAPPDATA%\Programs\Ori (or Program Files with ORI_SYSTEM=1), and adds
  that directory to your User PATH permanently.

  Designed to be piped into Invoke-Expression, like Scoop:

      irm https://raw.githubusercontent.com/raillen/ori-lang/master/tools/windows/get.ps1 | iex

  Optional environment variables (work with irm | iex):

      ORI_VERSION     Tag or semver without v, e.g. 0.3.5  (default: latest release)
      ORI_INSTALL_DIR Custom install directory
      ORI_FORCE       1 / true → overwrite existing install
      ORI_SYSTEM      1 / true → Program Files + Machine PATH (needs admin)
      ORI_REPO        GitHub owner/repo (default: raillen/ori-lang)
      ORI_BRANCH      Branch/tag for raw Install-Ori.ps1 (default: master)
      ORI_SKIP_DOCTOR 1 / true → skip `ori doctor` after install

  With parameters (download first, or use scriptblock):

      irm …/get.ps1 -OutFile get-ori.ps1
      .\get-ori.ps1 -Version 0.3.5 -Force

      & ([scriptblock]::Create((irm …/get.ps1))) -Version 0.3.5

.NOTES
  Requires network access to github.com. Does not require Rust.
  AOT (`ori compile`) still needs Visual Studio Build Tools (C++).
#>
[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$InstallDir = "",
    [string]$Repo = "raillen/ori-lang",
    [string]$Branch = "master",
    [switch]$Force,
    [switch]$System,
    [switch]$SkipDoctor
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

function Test-Truthy([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return $false }
    return @('1', 'true', 'yes', 'on') -contains $Value.Trim().ToLowerInvariant()
}

# Env vars for irm | iex (params win when explicitly passed on a local file run).
if ([string]::IsNullOrWhiteSpace($Version) -and $env:ORI_VERSION) { $Version = $env:ORI_VERSION.Trim() }
if ([string]::IsNullOrWhiteSpace($InstallDir) -and $env:ORI_INSTALL_DIR) { $InstallDir = $env:ORI_INSTALL_DIR.Trim() }
if ([string]::IsNullOrWhiteSpace($Repo) -or $Repo -eq "raillen/ori-lang") {
    if ($env:ORI_REPO) { $Repo = $env:ORI_REPO.Trim() }
}
if ($env:ORI_BRANCH) { $Branch = $env:ORI_BRANCH.Trim() }
if (-not $Force -and (Test-Truthy $env:ORI_FORCE)) { $Force = $true }
if (-not $System -and (Test-Truthy $env:ORI_SYSTEM)) { $System = $true }
if (-not $SkipDoctor -and (Test-Truthy $env:ORI_SKIP_DOCTOR)) { $SkipDoctor = $true }

Write-Host "[ori] Bootstrap installer (Scoop-style)" -ForegroundColor Cyan
Write-Host "[ori] Repo=$Repo branch=$Branch version=$(if ($Version) { $Version } else { 'latest' })"

# Prefer the full installer from the same branch so PATH + layout stay in sync.
$installerUrl = "https://raw.githubusercontent.com/$Repo/$Branch/tools/windows/Install-Ori.ps1"
$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("ori-get-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null
$installerPath = Join-Path $tmpDir "Install-Ori.ps1"

try {
    Write-Host "[ori] Fetching Install-Ori.ps1 from $installerUrl"
    $headers = @{ "User-Agent" = "ori-get-bootstrap" }
    if ($env:GITHUB_TOKEN) { $headers["Authorization"] = "Bearer $env:GITHUB_TOKEN" }

    try {
        Invoke-WebRequest -Uri $installerUrl -OutFile $installerPath -UseBasicParsing -Headers $headers
    } catch {
        Write-Warning "[ori] raw.githubusercontent failed ($($_.Exception.Message)); using embedded minimal installer."
        $installerPath = $null
    }

    if ($installerPath -and (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
        $invokeArgs = @{
            Repo = $Repo
        }
        if (-not [string]::IsNullOrWhiteSpace($Version)) { $invokeArgs.Version = $Version }
        if (-not [string]::IsNullOrWhiteSpace($InstallDir)) { $invokeArgs.InstallDir = $InstallDir }
        if ($Force) { $invokeArgs.Force = $true }
        if ($System) { $invokeArgs.System = $true }
        if ($SkipDoctor) { $invokeArgs.SkipDoctor = $true }

        Write-Host "[ori] Running Install-Ori.ps1…"
        # Do not call `exit` here: when this file is run via `irm | iex`, exit would
        # terminate the whole interactive session (Scoop-style bootstrap).
        & $installerPath @invokeArgs
        if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
            throw "Install-Ori.ps1 failed with exit code $LASTEXITCODE"
        }
        return
    }

    # ── Fallback: minimal self-contained path (no Install-Ori.ps1 available) ──
    Write-Host "[ori] Minimal install path (download release zip only)"
    $apiBase = "https://api.github.com/repos/$Repo/releases"
    $apiHeaders = @{
        "User-Agent" = "ori-get-bootstrap"
        "Accept"     = "application/vnd.github+json"
    }
    if ($env:GITHUB_TOKEN) { $apiHeaders["Authorization"] = "Bearer $env:GITHUB_TOKEN" }

    if ([string]::IsNullOrWhiteSpace($Version)) {
        $release = Invoke-RestMethod -Uri "$apiBase/latest" -Headers $apiHeaders
    } else {
        $tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
        $release = Invoke-RestMethod -Uri "$apiBase/tags/$tag" -Headers $apiHeaders
    }

    $asset = $release.assets | Where-Object {
        $_.name -match 'x86_64-pc-windows-msvc\.zip$'
    } | Select-Object -First 1
    if (-not $asset) {
        throw "No Windows MSVC zip on release $($release.tag_name)."
    }

    $zipPath = Join-Path $tmpDir $asset.name
    Write-Host "[ori] Downloading $($asset.browser_download_url)"
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath -UseBasicParsing -Headers $apiHeaders

    $extract = Join-Path $tmpDir "pkg"
    Expand-Archive -LiteralPath $zipPath -DestinationPath $extract -Force
    $packageRoot = $extract
    if (-not (Test-Path (Join-Path $packageRoot "ori.exe"))) {
        $child = Get-ChildItem -LiteralPath $extract -Directory | Select-Object -First 1
        if ($child) { $packageRoot = $child.FullName }
    }
    if (-not (Test-Path (Join-Path $packageRoot "ori.exe"))) {
        throw "Extracted package has no ori.exe"
    }

    if ([string]::IsNullOrWhiteSpace($InstallDir)) {
        if ($System) {
            $InstallDir = Join-Path ${env:ProgramFiles} "Ori"
        } else {
            $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\Ori"
        }
    }
    $InstallDir = [System.IO.Path]::GetFullPath($InstallDir)

    if ((Test-Path $InstallDir) -and -not $Force) {
        throw "Install dir exists: $InstallDir (set ORI_FORCE=1 or -Force)"
    }
    if (Test-Path $InstallDir) {
        Remove-Item -LiteralPath $InstallDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Path (Join-Path $packageRoot "*") -Destination $InstallDir -Recurse -Force

    $scope = if ($System) { "Machine" } else { "User" }
    $norm = $InstallDir.TrimEnd('\')
    $cur = [Environment]::GetEnvironmentVariable("Path", $scope)
    if ($null -eq $cur) { $cur = "" }
    $parts = @($cur -split ';' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    $found = $false
    foreach ($p in $parts) {
        try { if ([System.IO.Path]::GetFullPath($p).TrimEnd('\') -ieq $norm) { $found = $true; break } } catch {}
    }
    if (-not $found) {
        $newPath = if ($parts.Count -eq 0) { $norm } else { ($parts + $norm) -join ';' }
        [Environment]::SetEnvironmentVariable("Path", $newPath, $scope)
        Write-Host "[ori] Added to $scope PATH: $norm"
    }
    $env:Path = "$norm;$env:Path"

    $ori = Join-Path $InstallDir "ori.exe"
    Write-Host "[ori] Installed: $(& $ori --version 2>&1)"
    if (-not $SkipDoctor) {
        $env:ORI_REQUIRE_PACKAGED_RUNTIME = "1"
        & $ori doctor
    }
    Write-Host "[ori] Done. Open a new terminal if 'ori' is not found." -ForegroundColor Green
}
finally {
    if (Test-Path -LiteralPath $tmpDir) {
        Remove-Item -LiteralPath $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
