@echo off
setlocal
REM Double-click friendly launcher for Install-Ori.ps1 (same folder).
set "SCRIPT=%~dp0Install-Ori.ps1"
if not exist "%SCRIPT%" set "SCRIPT=%~dp0install.ps1"

where pwsh >nul 2>&1
if %ERRORLEVEL%==0 (
  pwsh -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" %*
  exit /b %ERRORLEVEL%
)

where powershell >nul 2>&1
if %ERRORLEVEL%==0 (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" %*
  exit /b %ERRORLEVEL%
)

echo PowerShell not found. Install PowerShell 7 or use Windows PowerShell.
exit /b 1
