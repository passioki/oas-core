@echo off
REM 停止编辑器（监听 8788 的 oasd.exe）
powershell -NoProfile -Command "$p = Get-NetTCPConnection -LocalPort 8788 -State Listen -ErrorAction SilentlyContinue; if ($p) { Stop-Process -Id $p.OwningProcess -Force; Write-Host 'editor stopped' } else { Write-Host 'editor not running' }"
