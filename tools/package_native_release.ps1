param(
    [string]$PackageRoot = "",
    [string]$ArchivePath = "",
    [switch]$SkipBuild,
    # Accept -Force as alias (common in CI / docs)
    [Alias("Force")]
    [switch]$Overwrite
)

$ErrorActionPreference = "Stop"
# PowerShell 7.4+ may treat native stderr as terminating errors under Stop.
$PSNativeCommandUseErrorActionPreference = $false

function Get-HostTriple {
    $text = (& rustc -Vv | Out-String)
    if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
        throw "rustc -Vv failed; install Rust before packaging Ori. Output: $text"
    }
    foreach ($line in ($text -split "`r?`n")) {
        if ($line -match '^host:\s*(.+)$') {
            return $Matches[1].Trim()
        }
    }
    throw "Could not detect the Rust host target from rustc -Vv. Output: $text"
}

function Get-WorkspaceVersion([string]$RepoRoot) {
    $cargoToml = Join-Path $RepoRoot "compiler/Cargo.toml"
    if (-not (Test-Path -LiteralPath $cargoToml -PathType Leaf)) {
        $cargoToml = Join-Path $RepoRoot "Cargo.toml"
    }
    $inWorkspacePackage = $false
    foreach ($line in Get-Content -LiteralPath $cargoToml) {
        $text = [string]$line
        if ($text.Trim() -eq "[workspace.package]") {
            $inWorkspacePackage = $true
            continue
        }
        if ($inWorkspacePackage -and $text.Trim().StartsWith("[")) {
            break
        }
        if ($inWorkspacePackage -and $text -match '^\s*version\s*=\s*"([^"]+)"') {
            return $Matches[1]
        }
    }
    throw "Could not find [workspace.package] version in $cargoToml."
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$version = Get-WorkspaceVersion $repoRoot
$hostTriple = Get-HostTriple
$compilerTarget = if ($env:CARGO_TARGET_DIR) {
    [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
} else {
    Join-Path $repoRoot "compiler/target"
}
$distRoot = Join-Path $compilerTarget "dist"

if ([string]::IsNullOrWhiteSpace($PackageRoot)) {
    $PackageRoot = Join-Path $distRoot "ori-$version-$hostTriple"
} else {
    $PackageRoot = [System.IO.Path]::GetFullPath($PackageRoot)
}

if ([string]::IsNullOrWhiteSpace($ArchivePath)) {
    $ArchivePath = Join-Path $distRoot "ori-$version-$hostTriple.zip"
} else {
    $ArchivePath = [System.IO.Path]::GetFullPath($ArchivePath)
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $ArchivePath) | Out-Null

$smokeArgs = @{
    PackageRoot = $PackageRoot
    KeepPackage = $true
}
if ($SkipBuild) {
    $smokeArgs.SkipBuild = $true
}

# Invoke smoke as a child script. Do not treat a null $LASTEXITCODE as failure:
# pure PowerShell success paths may leave LASTEXITCODE unset/null, and
# `$null -ne 0` is $true in PowerShell (which aborted packaging in ~seconds).
$smokeScript = Join-Path $PSScriptRoot "smoke_native_release.ps1"
if (-not (Test-Path -LiteralPath $smokeScript -PathType Leaf)) {
    throw "missing smoke script: $smokeScript"
}
& $smokeScript @smokeArgs
if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
    throw "smoke_native_release.ps1 failed with exit code $LASTEXITCODE."
}
if (-not (Test-Path -LiteralPath $PackageRoot -PathType Container)) {
    throw "package root missing after smoke: $PackageRoot"
}

if ((Test-Path -LiteralPath $ArchivePath) -and -not $Overwrite) {
    throw "Archive already exists at $ArchivePath. Pass -Overwrite to replace it."
}
if (Test-Path -LiteralPath $ArchivePath) {
    Remove-Item -LiteralPath $ArchivePath -Force
}

$PackageRoot = $PackageRoot.TrimEnd('\', '/')
Compress-Archive -LiteralPath $PackageRoot -DestinationPath $ArchivePath -Force

if (-not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
    throw "failed to create archive at $ArchivePath"
}

Write-Host "native release package: $PackageRoot"
Write-Host "native release archive: $ArchivePath"
exit 0
