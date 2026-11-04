@echo off
REM Stop OAS Studio (the oasd.exe listening on port 8788).
setlocal
powershell -NoProfile -Command "$p = Get-NetTCPConnection -LocalPort 8788 -State Listen -ErrorAction SilentlyContinue; if ($p) { Stop-Process -Id $p.OwningProcess -Force; Write-Host 'studio stopped' } else { Write-Host 'studio not running' }"
endlocal
