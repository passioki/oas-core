@echo off
REM OAS Studio launcher (authoring/editing; port 8788).
REM Studio is the develop/edit package, NOT the end-user Runner package.
setlocal
cd /d "%~dp0"
set "ADB_PATH=%~dp0adb\adb.exe"
if not exist "%ADB_PATH%" set "ADB_PATH=adb.exe"
if not exist "logs" mkdir logs
if not exist "runs" mkdir runs
if not exist "config\live.json" if exist "config\live.example.json" copy /Y "config\live.example.json" "config\live.json" >nul

start "" /B "%~dp0oasd.exe" studio ^
  --port 8788 ^
  --ui ui ^
  --tasks-dir tasks ^
  --runs-dir runs ^
  --library-dir library ^
  --assets-dir assets ^
  --frames-dir frames ^
  --model models\PP-OCRv6_rec_small.onnx ^
  --det-model models\PP-OCRv6_det_small.onnx ^
  --config config\live.json

echo.
echo Studio starting: http://127.0.0.1:8788/
echo Logs: logs\ ; stop with stop-studio.bat
endlocal
