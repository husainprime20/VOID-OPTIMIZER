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
echo   building VOID ^(native window + console companion^)...
echo   the first build downloads the UI and audio crates, then everything is offline.
cargo build --release --bins
if errorlevel 1 (
  echo.
  echo   build failed - open an issue with the output above.
  pause
  exit /b 1
)
copy /y target\release\void.exe VOID.exe >nul
copy /y target\release\void-cli.exe void-cli.exe >nul
echo.
echo   done -^> VOID.exe  ^(double-click it; it elevates itself and opens the window^)
echo          void-cli.exe  ^(optional console companion^)
echo.
pause
