[CmdletBinding()]
param(
    [string]$PackageRoot = "",
    [switch]$SkipBuild,
    [switch]$KeepPackage
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

function Get-HostTriple {
    $text = (& rustc -Vv | Out-String)
    if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
        throw "rustc -Vv failed; install Rust before running the native release smoke test. Output: $text"
    }
    foreach ($line in ($text -split "`r?`n")) {
        if ($line -match '^host:\s*(.+)$') {
            return $Matches[1].Trim()
        }
    }
    throw "Could not detect the Rust host target from rustc -Vv. Output: $text"
}

function Get-OriExeName {
    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        return "ori.exe"
    }

    return "ori"
}

function Get-LspExeName {
    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        return "ori-lsp.exe"
    }

    return "ori-lsp"
}

function Get-OutputExeName([string]$Name) {
    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        return "$Name.exe"
    }

    return $Name
}

function Invoke-Checked([scriptblock]$Command, [string]$Description) {
    & $Command
    if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$compilerRoot = Join-Path $repoRoot "compiler"
$hostTriple = Get-HostTriple
$targetRoot = if ($env:CARGO_TARGET_DIR) {
    [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
} else {
    Join-Path $compilerRoot "target"
}

if ([string]::IsNullOrWhiteSpace($PackageRoot)) {
    $PackageRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("ori-native-release-smoke-" + [System.Guid]::NewGuid().ToString("N"))
} else {
    $PackageRoot = [System.IO.Path]::GetFullPath($PackageRoot)
}

$packageRootPath = $PackageRoot
$oriExeName = Get-OriExeName
$lspExeName = Get-LspExeName
$sourceOri = Join-Path $targetRoot (Join-Path "release" $oriExeName)
$sourceLsp = Join-Path $targetRoot (Join-Path "release" $lspExeName)
$packageOri = Join-Path $packageRootPath $oriExeName
$packageLsp = Join-Path $packageRootPath $lspExeName
$examplesDir = Join-Path $packageRootPath "examples"
$runtimeDir = Join-Path $packageRootPath "runtime"
$stdlibDir = Join-Path $packageRootPath "stdlib"

Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        $manifest = Join-Path $repoRoot "compiler/Cargo.toml"
        Write-Host "Building release CLI from $manifest (target root: $targetRoot)"
        Invoke-Checked {
            # Cargo 1.95+: --manifest-path must follow the subcommand (not global).
            & cargo build --manifest-path $manifest -p ori-driver -p ori-lsp --release
        } "cargo build -p ori-driver -p ori-lsp --release"
    }

    if (-not (Test-Path -LiteralPath $sourceOri -PathType Leaf)) {
        throw "Release compiler was not found at $sourceOri."
    }
    if (-not (Test-Path -LiteralPath $sourceLsp -PathType Leaf)) {
        throw "Release LSP server was not found at $sourceLsp."
    }

    if (Test-Path -LiteralPath $packageRootPath) {
        Remove-Item -LiteralPath $packageRootPath -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $packageRootPath | Out-Null
    New-Item -ItemType Directory -Force -Path $examplesDir | Out-Null

    Copy-Item -LiteralPath $sourceOri -Destination $packageOri -Force
    Copy-Item -LiteralPath $sourceLsp -Destination $packageLsp -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot "stdlib") -Destination $stdlibDir -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot "examples/hello/main.orl") -Destination (Join-Path $examplesDir "hello.orl") -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot "examples/async_demo/main.orl") -Destination (Join-Path $examplesDir "async_demo.orl") -Force

    # Windows end-user installers (PATH + copy to %LOCALAPPDATA%\Programs\Ori).
    # Included only on Windows packages so the zip is self-contained.
    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        $winTools = Join-Path $PSScriptRoot "windows"
        $installPairs = @(
            @{ Src = "get.ps1"; Dest = "get.ps1" },
            @{ Src = "Install-Ori.ps1"; Dest = "Install-Ori.ps1" },
            @{ Src = "Install-Ori.ps1"; Dest = "install.ps1" },
            @{ Src = "Uninstall-Ori.ps1"; Dest = "Uninstall-Ori.ps1" },
            @{ Src = "Uninstall-Ori.ps1"; Dest = "uninstall.ps1" },
            @{ Src = "install.cmd"; Dest = "install.cmd" },
            @{ Src = "uninstall.cmd"; Dest = "uninstall.cmd" },
            @{ Src = "README.md"; Dest = "INSTALL-WINDOWS.md" }
        )
        foreach ($pair in $installPairs) {
            $src = Join-Path $winTools $pair.Src
            if (Test-Path -LiteralPath $src -PathType Leaf) {
                Copy-Item -LiteralPath $src -Destination (Join-Path $packageRootPath $pair.Dest) -Force
            } else {
                Write-Host "warning: missing Windows install helper $src"
            }
        }
    }

    $stageArgs = @{
        Target = $hostTriple
        Profile = "release"
        OutputRoot = $runtimeDir
    }
    if ($SkipBuild) {
        $stageArgs.SkipBuild = $true
    }
    Invoke-Checked { & (Join-Path $PSScriptRoot "stage_native_runtime.ps1") @stageArgs } "stage_native_runtime.ps1"

    # Build Ori fixtures as line arrays — avoid PowerShell here-strings that
    # contain `@test` (can confuse older parsers / editors).
    $testPath = Join-Path $examplesDir "package_smoke_test.orl"
    @(
        "module app.package_smoke"
        ""
        "import ori.test = test"
        "import ori.task = task"
        ""
        "@test"
        "package_smoke_test()"
        "    check 1 + 1 == 2"
        "    test.assert(true, `"package smoke test`")"
        "end"
        ""
        "@test"
        "async package_async_smoke_test()"
        "    await task.sleep(1)"
        "    test.assert(true, `"package async smoke test`")"
        "end"
    ) | Set-Content -LiteralPath $testPath -Encoding ascii

    $stdlibSmokePath = Join-Path $examplesDir "stdlib_package_smoke.orl"
    @(
        "module app.stdlib_package_smoke"
        ""
        "import ori.io = io"
        "import ori.string (trim_all)"
        ""
        "main()"
        "    io.print(trim_all(`"hello   packaged   stdlib`"))"
        "end"
    ) | Set-Content -LiteralPath $stdlibSmokePath -Encoding ascii

    $previousRequirePackagedRuntime = $env:ORI_REQUIRE_PACKAGED_RUNTIME
    $env:ORI_REQUIRE_PACKAGED_RUNTIME = "1"
    Push-Location $packageRootPath
    try {
        if (-not (Test-Path -LiteralPath $packageLsp -PathType Leaf)) {
            throw "packaged LSP server was not copied to $packageLsp."
        }
        if (-not (Test-Path -LiteralPath (Join-Path $stdlibDir "string.orl") -PathType Leaf)) {
            throw "packaged stdlib was not copied to $stdlibDir."
        }

        # Match tools/smoke_native_release.sh: CI may set ORI_PACKAGE_SMOKE_JIT_ONLY=1
        # to skip AOT compile/test (host linker flaky on some runners).
        $jitEnv = [string]$env:ORI_PACKAGE_SMOKE_JIT_ONLY
        $smokeJitOnly = @('1', 'true', 'yes', 'on') -contains $jitEnv.Trim().ToLowerInvariant()

        if (-not $smokeJitOnly) {
            $helloExe = Join-Path $packageRootPath (Get-OutputExeName "hello")
            Invoke-Checked { & $packageOri compile (Join-Path "examples" "hello.orl") --out $helloExe } "ori compile in packaged release folder"
            $helloOutput = & $helloExe
            if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
                throw "compiled hello executable failed with exit code $LASTEXITCODE."
            }
            if (($helloOutput -join "`n") -notmatch "The answer is: 42") {
                throw "compiled hello executable did not print the expected answer."
            }

            $asyncExe = Join-Path $packageRootPath (Get-OutputExeName "async_demo")
            Invoke-Checked { & $packageOri compile (Join-Path "examples" "async_demo.orl") --out $asyncExe } "ori compile async_demo in packaged release folder"
            $asyncOutput = & $asyncExe
            if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
                throw "compiled async_demo executable failed with exit code $LASTEXITCODE."
            }
            if (($asyncOutput -join "`n") -notmatch "42") {
                throw "compiled async_demo executable did not print the expected async answer."
            }

            $stdlibExe = Join-Path $packageRootPath (Get-OutputExeName "stdlib_package_smoke")
            Invoke-Checked { & $packageOri compile (Join-Path "examples" "stdlib_package_smoke.orl") --out $stdlibExe } "ori compile stdlib source module in packaged release folder"
            $stdlibOutput = & $stdlibExe
            if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
                throw "compiled stdlib_package_smoke executable failed with exit code $LASTEXITCODE."
            }
            if (($stdlibOutput -join "`n") -notmatch "hello packaged stdlib") {
                throw "compiled stdlib_package_smoke executable did not use the packaged stdlib."
            }

            Invoke-Checked { & $packageOri test (Join-Path "examples" "package_smoke_test.orl") } "ori test in packaged release folder"
        } else {
            Write-Host "ORI_PACKAGE_SMOKE_JIT_ONLY=1 — skipping AOT compile/test smoke (JIT + doctor only)"
        }

        $runtimeTripleDir = Join-Path $runtimeDir $hostTriple
        $cdylibName = if ($IsWindows -or $env:OS -eq "Windows_NT") {
            "ori_runtime.dll"
        } elseif ($hostTriple -like "*apple-darwin*") {
            "libori_runtime.dylib"
        } else {
            "libori_runtime.so"
        }
        $cdylibPath = Join-Path $runtimeTripleDir $cdylibName
        if (-not (Test-Path -LiteralPath $cdylibPath -PathType Leaf)) {
            throw "packaged runtime cdylib was not staged at $cdylibPath."
        }

        $jitOutput = & $packageOri run (Join-Path "examples" "hello.orl")
        if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
            throw "ori run (JIT default) failed with exit code $LASTEXITCODE."
        }
        if (($jitOutput -join "`n") -notmatch "The answer is: 42") {
            throw "ori run (JIT default) did not print the expected answer."
        }

        $previousEap = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        try {
            $doctorOutput = & $packageOri doctor 2>&1
        } finally {
            $ErrorActionPreference = $previousEap
        }
        if (($null -ne $LASTEXITCODE) -and ($LASTEXITCODE -ne 0)) {
            throw "ori doctor failed with exit code $LASTEXITCODE."
        }
        $doctorOutputRaw = ($doctorOutput | Out-String)
        if ($doctorOutputRaw -notmatch "SystemLinker" -and $doctorOutputRaw -notmatch "BundledRustLld" -and $doctorOutputRaw -notmatch "RustcDriver") {
            throw "ori doctor did not report the active linker strategy. Output was: $doctorOutputRaw"
        }
    } finally {
        Pop-Location
        if ($null -eq $previousRequirePackagedRuntime) {
            Remove-Item Env:\ORI_REQUIRE_PACKAGED_RUNTIME -ErrorAction SilentlyContinue
        } else {
            $env:ORI_REQUIRE_PACKAGED_RUNTIME = $previousRequirePackagedRuntime
        }
    }

    Write-Host "native release smoke passed: $packageRootPath"
} finally {
    Pop-Location
    if (-not $KeepPackage -and (Test-Path -LiteralPath $packageRootPath)) {
        Remove-Item -LiteralPath $packageRootPath -Recurse -Force
    }
}
