@echo off
REM 停止调度器（按命令行匹配 oasd.exe scheduler，单实例锁文件由程序自己清）
powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='oasd.exe'\" | Where-Object { $_.CommandLine -match 'scheduler' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force; Write-Host ('stopped ' + $_.ProcessId) }"
