@echo off
REM Deprecated compatibility launcher: `start-editor.bat` now delegates to
REM start-studio.bat (same port 8788, same behavior). Kept so existing
REM shortcuts/docs keep working after the Studio/Runner split.
call "%~dp0start-studio.bat"
