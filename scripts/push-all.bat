@echo off
rem OAS one-click commit/push wrapper (delegates to scripts\push-all.ps1).
rem Usage: push-all.bat [-Message "text"] [-Yes] [-DryRun] [-NoPush] [-SyncMain] [-Remote origin] [-Branch name]
setlocal
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0push-all.ps1" %*
exit /b %ERRORLEVEL%
