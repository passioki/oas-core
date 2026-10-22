@echo off
REM Stop the scheduler (match oasd.exe command line; single-instance lock is released by the process)
powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='oasd.exe'\" | Where-Object { $_.CommandLine -match 'scheduler' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force; Write-Host ('stopped ' + $_.ProcessId) }"
