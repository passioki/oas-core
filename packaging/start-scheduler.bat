@echo off
REM OAS V1.0 scheduler launcher (single-instance lock; log to logs\)
setlocal
cd /d "%~dp0"
set "ADB_PATH=%~dp0adb\adb.exe"
if not exist "%ADB_PATH%" set "ADB_PATH=adb.exe"
if not exist "logs" mkdir logs
if not exist "runs" mkdir runs

"%~dp0oasd.exe" scheduler ^
  --plan scheduler.json ^
  --state scheduler.state.json ^
  --tasks-dir tasks ^
  --library library ^
  --model models\PP-OCRv6_rec_small.onnx ^
  --det-model models\PP-OCRv6_det_small.onnx ^
  --report runs\scheduler-report.json ^
  --events logs\scheduler.events.jsonl ^
  --config config\live.json >> logs\scheduler.log 2>&1
endlocal
