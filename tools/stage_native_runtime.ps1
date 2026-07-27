[CmdletBinding()]
param(
    [string]$Target = "",
    # Default release (LANG-PERF residual / package parity). Override with -Profile debug
    # or $env:ORI_STAGE_PROFILE for faster ori-runtime rebuild loops.
    [ValidateSet("debug", "release")]
    [string]$Profile = $(if ($env:ORI_STAGE_PROFILE) { $env:ORI_STAGE_PROFILE } else { "release" }),
    [string]$OutputRoot = "",
    [switch]$SkipBuild,
    [switch]$SkipBundleLld
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

function Get-HostTriple {
    $text = (& rustc -Vv | Out-String)
    if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
        throw "rustc -Vv failed; install Rust or set -Target explicitly. Output: $text"
    }
    foreach ($line in ($text -split "`r?`n")) {
        if ($line -match '^host:\s*(.+)$') {
            return $Matches[1].Trim()
        }
    }
    throw "Could not detect the Rust host target from rustc -Vv. Output: $text"
}

function Get-RustLldPath {
    # 1. Explicit override via env var.
    if ($env:ORI_RUST_LLD -and (Test-Path -LiteralPath $env:ORI_RUST_LLD -PathType Leaf)) {
        return (Resolve-Path -LiteralPath $env:ORI_RUST_LLD).Path
    }

    # 2. Borrow from the Rust toolchain sysroot: <sysroot>/lib/rustlib/<host>/bin/rust-lld[.exe]
    $sysroot = (& rustc --print sysroot 2>$null)
    if ($LASTEXITCODE -eq 0 -and $sysroot) {
        $sysroot = $sysroot.Trim()
        $hostTriple = Get-HostTriple
        $exe = if ($IsWindows -or $env:OS -eq "Windows_NT") { "rust-lld.exe" } else { "rust-lld" }
        $candidate = Join-Path $sysroot (Join-Path "lib/rustlib/$hostTriple/bin" $exe)
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    # 3. PATH lookup as a last resort.
    $exe = if ($IsWindows -or $env:OS -eq "Windows_NT") { "rust-lld.exe" } else { "rust-lld" }
    $found = Get-Command -Name $exe -ErrorAction SilentlyContinue
    if ($found) {
        return $found.Source
    }

    return $null
}

function Get-RuntimeArtifactName([string]$TargetTriple) {
    if ($TargetTriple -like "*windows-msvc*") {
        return "ori_runtime.lib"
    }

    return "libori_runtime.a"
}

function Get-RuntimeCdylibName([string]$TargetTriple) {
    if ($TargetTriple -like "*windows-msvc*") {
        return "ori_runtime.dll"
    }
    if ($TargetTriple -like "*apple-darwin*") {
        return "libori_runtime.dylib"
    }

    return "libori_runtime.so"
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

    throw "Could not read workspace package version from Cargo.toml."
}

function Get-OriAbiVersion([string]$RepoRoot) {
    $runtimeSource = Join-Path $RepoRoot "compiler/crates/ori-runtime/src/lib.rs"
    foreach ($line in Get-Content -LiteralPath $runtimeSource) {
        $text = [string]$line
        if ($text -match 'pub\s+const\s+ORI_ABI_VERSION\s*:\s*&str\s*=\s*"([^"]+)"') {
            return $Matches[1]
        }
    }

    throw "Could not read ORI_ABI_VERSION from ori-runtime."
}

function Get-FallbackNativeStaticLibs([string]$TargetTriple) {
    if ($TargetTriple -like "*windows-msvc*") {
        return @(
            "legacy_stdio_definitions.lib",
            "kernel32.lib",
            "ntdll.lib",
            "userenv.lib",
            "ws2_32.lib",
            "dbghelp.lib",
            "/defaultlib:msvcrt"
        )
    }

    if ($TargetTriple -like "*linux*") {
        return @("-lpthread", "-ldl", "-lm", "-no-pie")
    }

    return @()
}

function Get-NativeStaticLibs([string]$TargetTriple, [string]$ProfileName) {
    $profileArgs = @()
    if ($ProfileName -eq "release") {
        $profileArgs += "--release"
    }

    $targetArgs = @("--target", $TargetTriple)
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & cargo rustc --manifest-path (Join-Path $RepoRoot "compiler/Cargo.toml") -p ori-runtime --lib @targetArgs @profileArgs -- --print native-static-libs 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousPreference
    }

    if ($exitCode -eq 0) {
        foreach ($line in $output) {
            $text = [string]$line
            $marker = "native-static-libs:"
            $index = $text.IndexOf($marker)
            if ($index -ge 0) {
                $libsText = $text.Substring($index + $marker.Length).Trim()
                if ($libsText.Length -gt 0) {
                    return @($libsText -split "\s+" | Where-Object { $_ -ne "" })
                }
            }
        }
    }

    return Get-FallbackNativeStaticLibs $TargetTriple
}

function Add-RequiredNativeLinkArgs([string]$TargetTriple, [string[]]$Libs) {
    $result = @($Libs)
    if ($TargetTriple -like "*linux*" -and -not ($result -contains "-no-pie")) {
        $result += "-no-pie"
    }

    return $result
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
if ([string]::IsNullOrWhiteSpace($Target)) {
    $Target = Get-HostTriple
}

$oriVersion = Get-WorkspaceVersion $repoRoot
$abiVersion = Get-OriAbiVersion $repoRoot
$artifact = Get-RuntimeArtifactName $Target
$cdylibArtifact = Get-RuntimeCdylibName $Target
$profileArgs = @()
if ($Profile -eq "release") {
    $profileArgs += "--release"
}
$targetArgs = @("--target", $Target)

Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        & cargo build --manifest-path (Join-Path $RepoRoot "compiler/Cargo.toml") -p ori-runtime --lib @targetArgs @profileArgs
        if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
            throw "cargo build -p ori-runtime --lib failed. failed."
        }
    }

    $targetRoot = if ($env:CARGO_TARGET_DIR) {
        [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    } else {
        Join-Path $repoRoot "compiler/target"
    }

    $candidates = @(
        (Join-Path $targetRoot (Join-Path $Target (Join-Path $Profile $artifact))),
        (Join-Path $targetRoot (Join-Path $Profile $artifact))
    )

    $source = $null
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            $source = Resolve-Path -LiteralPath $candidate
            break
        }
    }

    if ($null -eq $source) {
        throw "Runtime artifact $artifact was not found after build."
    }

    $cdylibCandidates = @(
        (Join-Path $targetRoot (Join-Path $Target (Join-Path $Profile $cdylibArtifact))),
        (Join-Path $targetRoot (Join-Path $Profile $cdylibArtifact))
    )

    $cdylibSource = $null
    foreach ($candidate in $cdylibCandidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            $cdylibSource = Resolve-Path -LiteralPath $candidate
            break
        }
    }

    $stageRoot = if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
        Join-Path $repoRoot "runtime"
    } else {
        [System.IO.Path]::GetFullPath($OutputRoot)
    }
    $targetDir = Join-Path $stageRoot $Target
    New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

    $dest = Join-Path $targetDir $artifact
    Copy-Item -LiteralPath $source -Destination $dest -Force

    if ($null -ne $cdylibSource) {
        $cdylibDest = Join-Path $targetDir $cdylibArtifact
        Copy-Item -LiteralPath $cdylibSource -Destination $cdylibDest -Force
        Write-Host "staged runtime cdylib: $cdylibDest"
    } else {
        Write-Warning "runtime cdylib $cdylibArtifact was not found after build; JIT mode (ORI_USE_JIT=1) will not be available."
    }

    $nativeStaticLibs = Add-RequiredNativeLinkArgs $Target (Get-NativeStaticLibs $Target $Profile)
    $metadata = [ordered]@{
        target = $Target
        runtime = $artifact
        runtime_cdylib = if ($null -ne $cdylibSource) { $cdylibArtifact } else { "" }
        ori_version = $oriVersion
        abi_version = $abiVersion
        profile = $Profile
        native_static_libs = @($nativeStaticLibs)
        generated_by = "tools/stage_native_runtime.ps1"
    }
    $metadataPath = Join-Path $targetDir "runtime-link.json"
    $metadata | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $metadataPath -Encoding UTF8

    Write-Host "staged runtime: $dest"
    Write-Host "metadata: $metadataPath"

    if (-not $SkipBundleLld) {
        $lldPath = Get-RustLldPath
        if ($null -ne $lldPath) {
            $binDir = Join-Path $stageRoot "bin"
            New-Item -ItemType Directory -Force -Path $binDir | Out-Null
            $lldDest = Join-Path $binDir (Split-Path -Leaf $lldPath)
            Copy-Item -LiteralPath $lldPath -Destination $lldDest -Force
            Write-Host "staged rust-lld: $lldDest"
        } else {
            Write-Warning "rust-lld not found; ORI_USE_BUNDLED_RUST_LLD will require ORI_RUST_LLD or rustc sysroot at link time."
        }
    }
} finally {
    Pop-Location
}
