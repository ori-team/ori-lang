[CmdletBinding()]
param(
    [string]$Target = "",
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

function Get-HostTriple {
    $rustcVersion = & rustc -Vv
    if ($LASTEXITCODE -ne 0) {
        throw "rustc -Vv failed; install Rust or pass -Target explicitly."
    }

    foreach ($line in $rustcVersion) {
        if ($line -like "host:*") {
            return $line.Substring(5).Trim()
        }
    }

    throw "Could not detect the Rust host target from rustc -Vv."
}

function Get-RuntimeArtifactName([string]$TargetTriple) {
    if ($TargetTriple -like "*windows-msvc*") {
        return "ori_runtime.lib"
    }

    return "libori_runtime.a"
}

function Resolve-SymbolTool {
    $candidates = @("llvm-nm", "llvm-nm.exe", "nm", "nm.exe")
    foreach ($candidate in $candidates) {
        $cmd = Get-Command $candidate -ErrorAction SilentlyContinue
        if ($null -ne $cmd) {
            return $cmd.Source
        }
    }

    throw "Could not find llvm-nm or nm. Install LLVM or binutils to inspect native runtime exports."
}

function Get-ManifestNativeSymbols([string]$RepoRoot) {
    $source = Get-Content -LiteralPath (Join-Path $RepoRoot "compiler/crates/ori-types/src/stdlib.rs") -Raw
    $symbols = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($match in [regex]::Matches($source, 'stdlib!\([\s\S]*?=>\s*"([^"]+)"[\s\S]*?\)')) {
        [void]$symbols.Add($match.Groups[1].Value)
    }
    return $symbols
}

function Get-BackendDirectOriImports([string]$RepoRoot) {
    $source = Get-Content -LiteralPath (Join-Path $RepoRoot "compiler/crates/ori-codegen/src/native_backend.rs") -Raw
    $symbols = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($match in [regex]::Matches($source, 'decl\(\s*"([^"]+)"')) {
        $symbol = $match.Groups[1].Value
        if ($symbol.StartsWith("ori_")) {
            [void]$symbols.Add($symbol)
        }
    }
    return $symbols
}

function Test-BackendDeclaresManifestLoop([string]$RepoRoot) {
    $source = Get-Content -LiteralPath (Join-Path $RepoRoot "compiler/crates/ori-codegen/src/native_backend.rs") -Raw
    return $source.Contains("stdlib_runtime_functions()") `
        -and $source.Contains("entry.runtime_symbol") `
        -and $source.Contains("stdlib_native_abi(entry.runtime_symbol)") `
        -and $source.Contains("decl(entry.runtime_symbol")
}

function Get-ExportedSymbols([string]$SymbolTool, [string]$ArtifactPath) {
    $output = & $SymbolTool -g --defined-only $ArtifactPath 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Symbol inspection failed with $SymbolTool.`n$output"
    }

    $symbols = [System.Collections.Generic.HashSet[string]]::new()
    foreach ($line in $output) {
        $text = [string]$line
        foreach ($match in [regex]::Matches($text, '\b_?(ori_[A-Za-z0-9_]+)\b')) {
            [void]$symbols.Add($match.Groups[1].Value)
        }
    }
    return $symbols
}

function Format-Missing([string]$Title, [string[]]$Symbols) {
    if ($Symbols.Count -eq 0) {
        return ""
    }

    $lines = @($Title)
    foreach ($symbol in $Symbols) {
        $lines += "  - $symbol"
    }
    return ($lines -join [Environment]::NewLine)
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
if ([string]::IsNullOrWhiteSpace($Target)) {
    $Target = Get-HostTriple
}

$profileArgs = @()
if ($Profile -eq "release") {
    $profileArgs += "--release"
}
$targetArgs = @("--target", $Target)

Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        & cargo build --manifest-path (Join-Path $RepoRoot "compiler/Cargo.toml") -p ori-runtime --lib @targetArgs @profileArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build -p ori-runtime --lib failed. failed."
        }
    }

    $artifact = Get-RuntimeArtifactName $Target
    $targetRoot = if ($env:CARGO_TARGET_DIR) {
        [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    } else {
        Join-Path $repoRoot "target"
    }
    $candidates = @(
        (Join-Path $targetRoot (Join-Path $Target (Join-Path $Profile $artifact))),
        (Join-Path $targetRoot (Join-Path $Profile $artifact))
    )
    $runtimeArtifact = $null
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            $runtimeArtifact = Resolve-Path -LiteralPath $candidate
            break
        }
    }
    if ($null -eq $runtimeArtifact) {
        throw "Runtime artifact $artifact was not found. Run tools/stage_native_runtime or build ori-runtime first."
    }

    $symbolTool = Resolve-SymbolTool
    $manifestSymbols = Get-ManifestNativeSymbols $repoRoot
    $backendSymbols = Get-BackendDirectOriImports $repoRoot
    $backendDeclaresManifestLoop = Test-BackendDeclaresManifestLoop $repoRoot
    $exportedSymbols = Get-ExportedSymbols $symbolTool $runtimeArtifact

    $missingManifestExports = @($manifestSymbols | Where-Object { -not $exportedSymbols.Contains($_) } | Sort-Object)
    $missingBackendExports = @($backendSymbols | Where-Object { -not $exportedSymbols.Contains($_) } | Sort-Object)

    if ($missingManifestExports.Count -gt 0 -or $missingBackendExports.Count -gt 0 -or -not $backendDeclaresManifestLoop) {
        $parts = @(
            (Format-Missing "manifest symbols missing from ori-runtime exports:" $missingManifestExports),
            (Format-Missing "backend ori_* imports missing from ori-runtime exports:" $missingBackendExports),
            $(if (-not $backendDeclaresManifestLoop) { "backend does not declare stdlib manifest symbols through the manifest loop" } else { "" })
        ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        throw ($parts -join ([Environment]::NewLine + [Environment]::NewLine))
    }

    Write-Host "native runtime export check passed"
    Write-Host "target: $Target"
    Write-Host "runtime: $runtimeArtifact"
    Write-Host "symbol tool: $symbolTool"
    Write-Host "manifest symbols: $($manifestSymbols.Count)"
    Write-Host "backend ori_* imports: $($backendSymbols.Count)"
    Write-Host "backend manifest declaration loop: present"
} finally {
    Pop-Location
}
