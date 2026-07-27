param(
    [int]$Iterations = 5
)

$ErrorActionPreference = "Stop"

function Get-RepoRoot {
    $scriptPath = Split-Path -Parent $PSCommandPath
    return (Resolve-Path (Join-Path $scriptPath "..")).Path
}

function New-Dir($Path) {
    if (-not (Test-Path $Path)) {
        New-Item -ItemType Directory -Force -Path $Path | Out-Null
    }
}

function Test-Command($Name) {
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-CommandVersionLine {
    param(
        [string]$Executable,
        [string[]]$Arguments
    )

    try {
        $output = & $Executable @Arguments 2>$null | Select-Object -First 1
        if ($null -eq $output) {
            return ""
        }
        return ([string]$output).Trim()
    } catch {
        return ""
    }
}

function ConvertTo-ProcessArgumentString {
    param([string[]]$Arguments)

    $quoted = foreach ($arg in $Arguments) {
        if ($arg -match '^[A-Za-z0-9_\-\.\\/:=]+$') {
            $arg
        } else {
            '"' + ($arg -replace '"', '\"') + '"'
        }
    }
    return ($quoted -join ' ')
}

function Invoke-MeasuredCommand {
    param(
        [string]$Name,
        [string]$Phase,
        [string]$FileName,
        [string]$Executable,
        [string[]]$Arguments,
        [string]$WorkingDirectory,
        [string]$ExpectedOutput,
        [int]$Iteration
    )

    $stdoutPath = Join-Path $TargetDir "$FileName-$Phase-$Iteration.stdout.txt"
    $stderrPath = Join-Path $TargetDir "$FileName-$Phase-$Iteration.stderr.txt"

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo.FileName = $Executable
    $process.StartInfo.Arguments = ConvertTo-ProcessArgumentString $Arguments
    $process.StartInfo.WorkingDirectory = $WorkingDirectory
    $process.StartInfo.RedirectStandardOutput = $true
    $process.StartInfo.RedirectStandardError = $true
    $process.StartInfo.UseShellExecute = $false

    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    [void]$process.Start()
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    $timer.Stop()

    Set-Content -Path $stdoutPath -Value $stdout -NoNewline
    Set-Content -Path $stderrPath -Value $stderr -NoNewline

    $normalized = $stdout -replace "`r`n", "`n"
    $outputOk = if ($ExpectedOutput.Length -eq 0) {
        $process.ExitCode -eq 0
    } else {
        $normalized.TrimEnd("`n") -eq $ExpectedOutput.TrimEnd("`n")
    }

    [pscustomobject]@{
        language = $Name
        phase = $Phase
        iteration = $Iteration
        exit_code = $process.ExitCode
        duration_ms = [math]::Round($timer.Elapsed.TotalMilliseconds, 3)
        output_ok = $outputOk
        stdout_bytes = [Text.Encoding]::UTF8.GetByteCount($stdout)
        stderr_bytes = [Text.Encoding]::UTF8.GetByteCount($stderr)
        command = "$Executable $($Arguments -join ' ')"
        stdout_path = $stdoutPath
        stderr_path = $stderrPath
    }
}

function Add-Result($Item) {
    $script:Results.Add($Item) | Out-Null
}

$Root = Get-RepoRoot
$BenchDir = Join-Path $Root "benchmarks/language-comparison"
$TargetDir = Join-Path $Root "target/language-comparison"
New-Dir $TargetDir

$Expected = @"
fib_acc=174264720000
sum_squares=2666686666700000
list_push_sum=9600440000
score=2666870531860000
"@

$Results = [System.Collections.Generic.List[object]]::new()
$ToolVersions = [ordered]@{}

if (Test-Command "cargo") {
    $ToolVersions["cargo"] = Get-CommandVersionLine "cargo" @("--version")
    $oriExe = Join-Path $TargetDir "workload_ori.exe"
    Add-Result (Invoke-MeasuredCommand "Ori" "build" "ori" "cargo" @(
        "run", "-p", "ori-driver", "--", "compile",
        (Join-Path $BenchDir "workload.orl"),
        "--out", $oriExe
    ) $Root "" 0)
    if (Test-Path $oriExe) {
        for ($i = 1; $i -le $Iterations; $i++) {
            Add-Result (Invoke-MeasuredCommand "Ori" "run" "ori" $oriExe @() $Root $Expected $i)
        }
    }
}

if (Test-Command "rustc") {
    $ToolVersions["rustc"] = Get-CommandVersionLine "rustc" @("--version")
    $rustExe = Join-Path $TargetDir "workload_rust.exe"
    Add-Result (Invoke-MeasuredCommand "Rust" "build" "rust" "rustc" @(
        "-C", "opt-level=3",
        "-o", $rustExe,
        (Join-Path $BenchDir "workload.rs")
    ) $Root "" 0)
    if (Test-Path $rustExe) {
        for ($i = 1; $i -le $Iterations; $i++) {
            Add-Result (Invoke-MeasuredCommand "Rust" "run" "rust" $rustExe @() $Root $Expected $i)
        }
    }
}

if (Test-Command "gcc") {
    $ToolVersions["gcc"] = Get-CommandVersionLine "gcc" @("--version")
    $cExe = Join-Path $TargetDir "workload_c.exe"
    Add-Result (Invoke-MeasuredCommand "C" "build" "c" "gcc" @(
        "-O2", "-std=c11",
        "-o", $cExe,
        (Join-Path $BenchDir "workload.c")
    ) $Root "" 0)
    if (Test-Path $cExe) {
        for ($i = 1; $i -le $Iterations; $i++) {
            Add-Result (Invoke-MeasuredCommand "C" "run" "c" $cExe @() $Root $Expected $i)
        }
    }
}

if (Test-Command "python") {
    $ToolVersions["python"] = Get-CommandVersionLine "python" @("--version")
    for ($i = 1; $i -le $Iterations; $i++) {
        Add-Result (Invoke-MeasuredCommand "Python" "run" "python" "python" @(
            (Join-Path $BenchDir "workload.py")
        ) $Root $Expected $i)
    }
}

if (Test-Command "node") {
    $ToolVersions["node"] = Get-CommandVersionLine "node" @("--version")
    for ($i = 1; $i -le $Iterations; $i++) {
        Add-Result (Invoke-MeasuredCommand "Node" "run" "node" "node" @(
            (Join-Path $BenchDir "workload.js")
        ) $Root $Expected $i)
    }
}

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$csvPath = Join-Path $TargetDir "language-comparison-$stamp.csv"
$summaryPath = Join-Path $TargetDir "language-comparison-$stamp.txt"

$Results | Export-Csv -NoTypeInformation -Path $csvPath

$runGroups = $Results |
    Where-Object { $_.phase -eq "run" -and $_.exit_code -eq 0 -and $_.output_ok } |
    Group-Object language

$summaryRows = foreach ($group in $runGroups) {
    $runs = $group.Group
    $best = ($runs | Measure-Object duration_ms -Minimum).Minimum
    $avg = ($runs | Measure-Object duration_ms -Average).Average
    $build = ($Results |
        Where-Object { $_.language -eq $group.Name -and $_.phase -eq "build" } |
        Select-Object -First 1).duration_ms
    if ($null -eq $build) {
        $build = 0
    }
    [pscustomobject]@{
        language = $group.Name
        build_ms = [math]::Round([double]$build, 3)
        best_run_ms = [math]::Round([double]$best, 3)
        avg_run_ms = [math]::Round([double]$avg, 3)
        successful_runs = $runs.Count
    }
}

$oriBest = ($summaryRows | Where-Object language -eq "Ori" | Select-Object -First 1).best_run_ms
$summary = @()
$summary += "Ori language comparison"
$summary += "iterations: $Iterations"
$summary += "csv: $csvPath"
$summary += ""
$summary += "Expected output:"
$summary += $Expected.TrimEnd()
$summary += ""
$summary += "Tool versions:"
foreach ($tool in $ToolVersions.Keys) {
    if ($ToolVersions[$tool].Length -gt 0) {
        $summary += "- ${tool}: $($ToolVersions[$tool])"
    }
}
$summary += ""
$summary += "Results:"
foreach ($row in ($summaryRows | Sort-Object best_run_ms)) {
    $relative = ""
    if ($oriBest -and $row.best_run_ms -gt 0) {
        $relative = " | vs Ori run: " + [math]::Round($row.best_run_ms / $oriBest, 3) + "x"
    }
    $summary += "- $($row.language): build=$($row.build_ms) ms, best_run=$($row.best_run_ms) ms, avg_run=$($row.avg_run_ms) ms, ok_runs=$($row.successful_runs)$relative"
}

$summary += ""
$summary += "Notes:"
$summary += "- Process startup is included in run time."
$summary += "- Ori uses the current native backend."
$summary += "- Rust and C are compiled with local optimization flags."
$summary += "- Python and Node run through their installed runtimes."
$summary += "- This measures these workloads only; it is not a full language maturity ranking."

Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)

Write-Host "comparison_csv=$csvPath"
Write-Host "summary_txt=$summaryPath"
