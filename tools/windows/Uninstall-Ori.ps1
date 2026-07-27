#Requires -Version 5.1
<#
.SYNOPSIS
  Uninstall Ori from Windows and remove it from PATH.

.DESCRIPTION
  Removes the install directory (default %LOCALAPPDATA%\Programs\Ori) and
  strips matching entries from User or Machine PATH.

.EXAMPLE
  .\uninstall.ps1
  pwsh -File tools/windows/Uninstall-Ori.ps1 -InstallDir "$env:LOCALAPPDATA\Programs\Ori"
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [string]$InstallDir = "",
    [switch]$System,
    [switch]$KeepFiles
)

$ErrorActionPreference = "Stop"

function Write-Info([string]$Message) { Write-Host "[ori-uninstall] $Message" }

function Test-IsAdministrator {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($id)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
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
        if ($PSCmdlet.ShouldProcess("PATH ($Scope)", "Remove $normalized")) {
            [Environment]::SetEnvironmentVariable("Path", ($kept -join ';'), $Scope)
            Write-Info "Removed from $Scope PATH: $normalized"
        }
    } else {
        Write-Info "PATH ($Scope) did not contain $normalized"
    }
}

if ($System -and -not (Test-IsAdministrator)) {
    throw "-System requires an elevated PowerShell (Run as administrator)."
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    # Prefer manifest next to this script if uninstall is run from install dir.
    $manifestBeside = Join-Path $PSScriptRoot "ori-install.json"
    if (Test-Path -LiteralPath $manifestBeside -PathType Leaf) {
        try {
            $m = Get-Content -LiteralPath $manifestBeside -Raw | ConvertFrom-Json
            if ($m.installDir) { $InstallDir = [string]$m.installDir }
            if ($m.pathScope -eq "Machine") { $System = $true }
        } catch {
            # ignore
        }
    }
}
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    if ($System) {
        $InstallDir = Join-Path ${env:ProgramFiles} "Ori"
    } else {
        $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\Ori"
    }
}

$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$pathScope = if ($System) { "Machine" } else { "User" }

# If manifest exists at default location, honor its pathScope.
$manifestPath = Join-Path $InstallDir "ori-install.json"
if (Test-Path -LiteralPath $manifestPath -PathType Leaf) {
    try {
        $m = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        if ($m.pathScope -eq "Machine" -or $m.pathScope -eq "User") {
            $pathScope = [string]$m.pathScope
            if ($pathScope -eq "Machine" -and -not (Test-IsAdministrator)) {
                throw "This install used Machine PATH; re-run uninstall as Administrator."
            }
        }
        if ($m.installDir) {
            $InstallDir = [System.IO.Path]::GetFullPath([string]$m.installDir)
        }
    } catch {
        if ($_.Exception.Message -match "Administrator") { throw }
        Write-Info "Could not parse ori-install.json; using defaults."
    }
}

Write-Info "Uninstalling Ori from $InstallDir (PATH scope: $pathScope)"
Remove-PathEntry -Directory $InstallDir -Scope $pathScope

if (-not $KeepFiles) {
    if (Test-Path -LiteralPath $InstallDir) {
        if ($PSCmdlet.ShouldProcess($InstallDir, "Remove install directory")) {
            Remove-Item -LiteralPath $InstallDir -Recurse -Force
            Write-Info "Removed $InstallDir"
        }
    } else {
        Write-Info "Install directory already absent: $InstallDir"
    }
} else {
    Write-Info "Kept files (-KeepFiles)."
}

Write-Host "Ori uninstall complete. Open a new terminal so PATH refreshes." -ForegroundColor Green
