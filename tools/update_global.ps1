# Build a release package on this Windows machine and install it globally.
# Prefer tools/windows/Install-Ori.ps1 for end users (no Rust required).
#
# Usage (developer machine with Rust + MSVC):
#   pwsh -File tools/update_global.ps1
#   pwsh -File tools/update_global.ps1 -InstallDir C:\Ori -Force

param(
    [string]$InstallDir = "",
    [switch]$SkipBuild,
    [switch]$Force,
    [switch]$System
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$packScript = Join-Path $PSScriptRoot "package_native_release.ps1"
$installScript = Join-Path $PSScriptRoot "windows\Install-Ori.ps1"

if (-not (Test-Path -LiteralPath $packScript)) {
    throw "missing $packScript"
}
if (-not (Test-Path -LiteralPath $installScript)) {
    throw "missing $installScript"
}

Write-Host "Packaging Ori release (smoke + zip)..."
$packArgs = @{ Overwrite = $true }
if ($SkipBuild) { $packArgs.SkipBuild = $true }
& $packScript @packArgs
if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
    throw "package_native_release.ps1 failed ($LASTEXITCODE)"
}

# Discover the archive next to the default dist layout.
$versionLine = Select-String -Path (Join-Path $repoRoot "compiler\Cargo.toml") -Pattern '^\s*version\s*=\s*"([^"]+)"' |
    Select-Object -First 1
if (-not $versionLine) {
    throw "could not read workspace version from compiler/Cargo.toml"
}
# Prefer the newest matching zip under compiler/target/dist
$dist = Join-Path $repoRoot "compiler\target\dist"
$zip = Get-ChildItem -LiteralPath $dist -Filter "ori-*-x86_64-pc-windows-msvc.zip" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $zip) {
    $zip = Get-ChildItem -LiteralPath $dist -Filter "ori-*.zip" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
}
if (-not $zip) {
    throw "No release zip found under $dist after packaging."
}

Write-Host "Installing from $($zip.FullName)..."
$installArgs = @{
    ZipPath = $zip.FullName
}
if (-not [string]::IsNullOrWhiteSpace($InstallDir)) {
    $installArgs.InstallDir = $InstallDir
}
if ($Force) { $installArgs.Force = $true }
if ($System) { $installArgs.System = $true }

& $installScript @installArgs
if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
    throw "Install-Ori.ps1 failed ($LASTEXITCODE)"
}

Write-Host "Global update complete."
