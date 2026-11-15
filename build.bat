@echo off
setlocal
title VOID // build
where cargo >nul 2>nul
if errorlevel 1 (
  echo.
  echo   Rust is not installed. Get it from https://rustup.rs  ^(one command, no admin^)
  echo.
  pause
  exit /b 1
)
echo   building VOID in release mode ^(zero dependencies, fully offline^)...
cargo build --release
if errorlevel 1 (
  echo.
  echo   build failed - open an issue with the output above.
  pause
  exit /b 1
)
copy /y target\release\void.exe void.exe >nul
echo.
echo   done -^> void.exe  ^(drag it anywhere, it is a single file^)
echo   run it as Administrator for kernel-tier tweaks.
echo.
pause
