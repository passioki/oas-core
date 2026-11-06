@echo off
REM OAS source-tree Runner launcher (developer use).
REM Builds when needed and starts the Runner surface only:
REM   http://127.0.0.1:8789/   (embedded scheduler; no Studio authoring UI)
REM
REM Usage:
REM   start-runner.bat                start Runner on 8789 and open the browser
REM   start-runner.bat -NoOpen        do not open a browser
REM   start-runner.bat -NoScheduler   disable the embedded scheduler
REM   start-runner.bat -Release       use the release build
REM   start-runner.bat -SkipBuild     reuse target\debug\oasd.exe
REM   start-runner.bat -DryRun        only print the plan (smoke check)
REM Any extra arguments are forwarded to scripts\dev-serve.bat.
setlocal
cd /d "%~dp0"
if not exist "scripts\dev-serve.bat" goto no_script
echo [OAS] Starting Runner - http://127.0.0.1:8789/
call "scripts\dev-serve.bat" -Surface runner %*
set "RC=%ERRORLEVEL%"
if not "%RC%"=="0" goto failed
exit /b 0

:no_script
echo [OAS] scripts\dev-serve.bat not found.
echo [OAS] Run this script from the oas-core repository root.
pause
exit /b 1

:failed
echo.
echo [OAS] Runner failed to start. Exit code: %RC%
echo [OAS] Check the build/server output above, or run:
echo       scripts\dev-serve.bat -Surface runner
pause
exit /b %RC%
