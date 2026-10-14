@echo off
setlocal
cd /d "%~dp0..\native\recorder_core"

if not exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    echo Cargo is required. Install Rust from https://rustup.rs
    exit /b 1
)

cargo build --release
if errorlevel 1 exit /b 1

if not exist "%~dp0..\bin" mkdir "%~dp0..\bin"
copy /Y target\release\recorder_core.dll "%~dp0..\bin\recorder_core.dll" >nul
echo Native core copied to bin\recorder_core.dll
