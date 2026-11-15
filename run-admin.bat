@echo off
rem Launch VOID elevated. The exe itself has no manifest, so we elevate here.
if not exist "%~dp0void.exe" (
  echo Build first: build.bat
  pause
  exit /b 1
)
powershell -NoProfile -Command "Start-Process -FilePath '%~dp0void.exe' -WorkingDirectory '%~dp0' -Verb RunAs"
