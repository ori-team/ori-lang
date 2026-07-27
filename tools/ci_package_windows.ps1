# CI helper: build Windows release zip with verbose logging.
# Usage (from repo root):
#   pwsh -File tools/ci_package_windows.ps1 -Tag v0.3.5 -TargetName x86_64-pc-windows-msvc

param(
    [Parameter(Mandatory = $true)]
    [string]$Tag,
    [string]$TargetName = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

function Write-Step([string]$Message) {
    Write-Host ("::group::{0}" -f $Message)
    Write-Host ("[{0}] {1}" -f (Get-Date -Format o), $Message)
}

function Write-StepEnd {
    Write-Host "::endgroup::"
}

try {
    if ($Tag -notmatch '^v') { $Tag = "v$Tag" }

    $repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
    Set-Location $repoRoot
    Write-Host "repoRoot=$repoRoot"
    Write-Host "tag=$Tag target=$TargetName"
    Write-Host "ORI_PACKAGE_SMOKE_JIT_ONLY=$env:ORI_PACKAGE_SMOKE_JIT_ONLY"

    Write-Step "rustc / cargo"
    & rustc -Vv
    if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) { throw "rustc -Vv failed ($LASTEXITCODE)" }
    & cargo -V
    if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) { throw "cargo -V failed ($LASTEXITCODE)" }
    Write-StepEnd

    $dist = Join-Path $repoRoot "compiler\target\dist"
    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    $root = Join-Path $dist ("ori-" + $TargetName)
    $archive = Join-Path $dist ("ori-" + $Tag + "-" + $TargetName + ".zip")
    Write-Host "packageRoot=$root"
    Write-Host "archive=$archive"

    Write-Step "package_native_release.ps1"
    $pack = Join-Path $repoRoot "tools\package_native_release.ps1"
    if (-not (Test-Path -LiteralPath $pack)) { throw "missing $pack" }

    # Call with explicit switches; capture any terminating errors.
    & $pack -PackageRoot $root -ArchivePath $archive -Force
    $packExit = $LASTEXITCODE
    Write-Host "package_native_release LASTEXITCODE=$packExit"
    Write-StepEnd

    if (-not (Test-Path -LiteralPath $root -PathType Container)) {
        throw "package root not created: $root"
    }
    if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) {
        throw "archive not created: $archive"
    }

    Write-Step "package contents"
    Get-ChildItem -LiteralPath $root | Format-Table Name, Length
    Get-ChildItem -LiteralPath $dist | Format-Table Name, Length
    Write-StepEnd

    Write-Host "OK archive=$archive size=$((Get-Item -LiteralPath $archive).Length)"
    exit 0
}
catch {
    Write-Host "::error::$($_.Exception.Message)"
    Write-Host $_
    Write-Host $_.ScriptStackTrace
    exit 1
}
