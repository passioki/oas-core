@echo off
REM OAS V1.2.1 editor launcher (port 8788; avoids old OAS 22287)
setlocal
cd /d "%~dp0"
set "ADB_PATH=%~dp0adb\adb.exe"
if not exist "%ADB_PATH%" set "ADB_PATH=adb.exe"
if not exist "logs" mkdir logs
if not exist "runs" mkdir runs

start "" /B "%~dp0oasd.exe" serve ^
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
echo Editor starting: http://127.0.0.1:8788/
echo Logs: logs\ ; stop with stop-editor.bat
endlocal
