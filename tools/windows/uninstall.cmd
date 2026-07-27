@echo off
setlocal
set "SCRIPT=%~dp0Uninstall-Ori.ps1"
if not exist "%SCRIPT%" set "SCRIPT=%~dp0uninstall.ps1"

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

echo PowerShell not found.
exit /b 1
