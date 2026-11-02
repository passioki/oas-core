@echo off
REM OAS V1.0 scheduler launcher (single-instance lock; log to logs\)
REM --plan   = the SCHEDULE plan file: scheduler.json (schema v4; v3 is read as a
REM            legacy document and upgraded in memory).
REM --config = the GLOBAL live deployment config (config\live.json), consumed by
REM            main.rs before the subcommand is dispatched. It is NOT the
REM            schedule plan: pointing it at scheduler.json fails immediately
REM            with "config error ... unknown field `config`" (exit 2) and no
REM            plan runs.
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
