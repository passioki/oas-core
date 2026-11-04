@echo off
REM Stop the OAS Runner (the oasd.exe listening on port 8789).
setlocal
powershell -NoProfile -Command "$p = Get-NetTCPConnection -LocalPort 8789 -State Listen -ErrorAction SilentlyContinue; if ($p) { Stop-Process -Id $p.OwningProcess -Force; Write-Host 'runner stopped' } else { Write-Host 'runner not running' }"
endlocal
