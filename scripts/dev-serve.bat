@echo off
REM OAS dev launcher wrapper (one binary, two surfaces):
REM   dev-serve.bat                    -> Studio  http://127.0.0.1:8788/
REM   dev-serve.bat -Surface runner    -> Runner  http://127.0.0.1:8789/
REM All arguments are forwarded to dev-serve.ps1:
REM   -Port <n> -Release -SkipBuild -NoOpen -NoScheduler -DryRun
setlocal
where pwsh.exe >nul 2>nul
if %ERRORLEVEL%==0 (
  pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0dev-serve.ps1" %*
) else (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0dev-serve.ps1" %*
)
exit /b %ERRORLEVEL%
