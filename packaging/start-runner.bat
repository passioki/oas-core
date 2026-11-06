@echo off
REM OAS Runner launcher (use-side package; HTTP UI on port 8789).
REM Requires oasd.exe with the `runner` subcommand (embeds the scheduler; no
REM separate start-scheduler.bat needed). Only ui/runner.* is served here - the
REM Studio authoring UI is not part of this package.
REM
REM Usage: start-runner.bat [-NoOpen]
REM   -NoOpen   do not open the browser after startup
setlocal
cd /d "%~dp0"
set "ADB_PATH=%~dp0adb\adb.exe"
if not exist "%ADB_PATH%" set "ADB_PATH=adb.exe"
if not exist "logs" mkdir logs
if not exist "runs" mkdir runs
if not exist "packages" mkdir packages
if not exist "tasks" mkdir tasks
if not exist "assets" mkdir assets
if not exist "config\live.json" if exist "config\live.example.json" copy /Y "config\live.example.json" "config\live.json" >nul

REM ---- preflight checks (clear message instead of a silent no-op window) ----
set "OPEN=1"
if /I "%~1"=="-NoOpen" set "OPEN=0"
if not exist "%~dp0oasd.exe" (
  echo [OAS] oasd.exe not found in "%~dp0".
  echo [OAS] Extract the whole package to a writable folder and run this script again.
  pause
  exit /b 1
)
if not exist "models\PP-OCRv6_rec_small.onnx" echo [OAS] Warning: models\PP-OCRv6_rec_small.onnx missing - OCR will be unavailable.
if not exist "models\PP-OCRv6_det_small.onnx" echo [OAS] Warning: models\PP-OCRv6_det_small.onnx missing - OCR line detection will be unavailable.
if not exist "config\live.json" (
  echo [OAS] config\live.json not found and no config\live.example.json to copy.
  pause
  exit /b 1
)
for /f "tokens=*" %%L in ('netstat -ano ^| findstr /R /C:"TCP.*:8789 .*LISTENING"') do set "PORT_BUSY=1"
if defined PORT_BUSY (
  echo [OAS] Port 8789 is already in use. Run stop-runner.bat first, or close the other Runner.
  pause
  exit /b 1
)

start "" /B "%~dp0oasd.exe" runner ^
  --port 8789 ^
  --ui ui ^
  --tasks-dir tasks ^
  --runs-dir runs ^
  --assets-dir assets ^
  --model models\PP-OCRv6_rec_small.onnx ^
  --det-model models\PP-OCRv6_det_small.onnx ^
  --config config\live.json >> logs\runner.log 2>&1

REM ---- give it a moment, then verify the port actually came up ----
timeout /t 2 /nobreak >nul
set "PORT_OK="
for /f "tokens=*" %%L in ('netstat -ano ^| findstr /R /C:"TCP.*:8789 .*LISTENING"') do set "PORT_OK=1"
if not defined PORT_OK (
  echo.
  echo [OAS] Runner did NOT start (port 8789 is not listening).
  echo [OAS] Last lines of logs\runner.log:
  echo ------------------------------------------------------------
  powershell -NoProfile -Command "Get-Content -LiteralPath 'logs\runner.log' -Tail 20 -ErrorAction SilentlyContinue"
  echo ------------------------------------------------------------
  pause
  exit /b 1
)

echo.
echo Runner started: http://127.0.0.1:8789/
echo Config: config\live.json  (edit device.serial / device.adb_path first)
echo Logs:   logs\runner.log
echo Stop:   stop-runner.bat  (or close this window)
if "%OPEN%"=="1" start "" "http://127.0.0.1:8789/"
endlocal
exit /b 0
