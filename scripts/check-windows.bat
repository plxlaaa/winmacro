@echo off
REM ===========================================================================
REM  Windows 编译预检（Windows 版）
REM
REM  用途：在推 tag 之前，一次性抓出所有 Windows-only 的编译错误，
REM        避免等 CI 跑 10 分钟才发现 #[cfg(windows)] 里的问题。
REM
REM  用法：scripts\check-windows.bat
REM
REM  注意：本脚本需要 Windows + MSVC 工具链（Visual Studio Build Tools）。
REM        在 Linux / macOS 上请改用 scripts/check-windows.sh，
REM        那个版本会走 cargo-xwin 方案处理含 C 依赖的 crate。
REM ===========================================================================
setlocal enabledelayedexpansion

set TARGET=x86_64-pc-windows-msvc
set ROOT=%~dp0..

echo ============================================================
echo  WinMacro Windows 编译预检
echo  target: %TARGET%
echo ============================================================
echo.

REM --- 0. 确保 msvc target 存在 ---------------------------------------------
rustup target list --installed | findstr /C:"%TARGET%" >nul 2>&1
if errorlevel 1 (
    echo [0/5] 安装 Rust target %TARGET% ...
    rustup target add %TARGET%
    if errorlevel 1 (
        echo [FAIL] rustup target add 失败
        exit /b 1
    )
) else (
    echo [0/5] Rust target %TARGET% 已安装
)
echo.

REM --- 1. Rust 单元测试（Linux/当前平台）-----------------------------------
echo [1/5] cargo test - winmacro-logic ...
cargo test --manifest-path "%ROOT%\app-tauri\logic\Cargo.toml"
if errorlevel 1 (
    echo [FAIL] winmacro-logic 测试失败
    exit /b 1
)
echo.

echo [2/5] cargo test - recorder_core ...
cargo test --manifest-path "%ROOT%\native\recorder_core\Cargo.toml"
if errorlevel 1 (
    echo [FAIL] recorder_core 测试失败
    exit /b 1
)
echo.

REM --- 2. Windows target 编译检查（抓 #[cfg(windows)] 里的错）---------------
echo [3/5] cargo check --target %TARGET% - recorder_core ...
cargo check --target %TARGET% --all-targets --manifest-path "%ROOT%\native\recorder_core\Cargo.toml"
if errorlevel 1 (
    echo [FAIL] recorder_core Windows 编译失败
    exit /b 1
)
echo.

echo [4/5] cargo check --target %TARGET% - src-tauri （较慢，Tauri 依赖树大）...
cargo check --target %TARGET% --manifest-path "%ROOT%\app-tauri\src-tauri\Cargo.toml"
if errorlevel 1 (
    echo [FAIL] src-tauri Windows 编译失败
    exit /b 1
)
echo.

REM --- 3. 前端类型检查 + JS 单测 --------------------------------------------
echo [5/5] vue-tsc --noEmit + npm run test:js ...
pushd "%ROOT%\app-tauri"
call npm install --no-audit --no-fund
if errorlevel 1 (
    popd
    echo [FAIL] npm install 失败
    exit /b 1
)
call npx vue-tsc --noEmit
if errorlevel 1 (
    popd
    echo [FAIL] TypeScript 类型检查失败
    exit /b 1
)
call npm run test:js
if errorlevel 1 (
    popd
    echo [FAIL] JS 单测失败
    exit /b 1
)
popd
echo.

echo ============================================================
echo  全部通过。可以打 tag 了：
echo    git tag -a v2.5.0 -m "..."
echo    git push origin v2.5.0
echo ============================================================
endlocal
