@echo off
REM Stop the editor (the oasd.exe listening on port 8788)
powershell -NoProfile -Command "$p = Get-NetTCPConnection -LocalPort 8788 -State Listen -ErrorAction SilentlyContinue; if ($p) { Stop-Process -Id $p.OwningProcess -Force; Write-Host 'editor stopped' } else { Write-Host 'editor not running' }"
