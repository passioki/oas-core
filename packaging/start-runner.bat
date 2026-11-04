@echo off
REM OAS Runner launcher (use-side only; HTTP UI on port 8789).
REM Requires an oasd.exe with the PR1 `runner` subcommand. The Runner embeds the
REM scheduler (no separate start-scheduler.bat needed) and only serves
REM ui/runner.* - the Studio authoring UI is not part of this package.
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

start "" /B "%~dp0oasd.exe" runner ^
  --port 8789 ^
  --ui ui ^
  --tasks-dir tasks ^
  --runs-dir runs ^
  --assets-dir assets ^
  --model models\PP-OCRv6_rec_small.onnx ^
  --det-model models\PP-OCRv6_det_small.onnx ^
  --config config\live.json >> logs\runner.log 2>&1

echo.
echo Runner starting: http://127.0.0.1:8789/
echo Config: config\live.json  (edit device.serial / device.adb_path first)
echo Logs:   logs\runner.log
echo Stop:   stop-runner.bat  (or close this window)
endlocal
