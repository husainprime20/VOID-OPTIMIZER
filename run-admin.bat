@echo off
rem Launch VOID elevated. VOID.exe already asks for elevation itself — this is
rem kept for people who prefer an explicit shortcut.
if not exist "%~dp0VOID.exe" (
  echo Build first: build.bat
  pause
  exit /b 1
)
powershell -NoProfile -Command "Start-Process -FilePath '%~dp0VOID.exe' -WorkingDirectory '%~dp0' -Verb RunAs"
