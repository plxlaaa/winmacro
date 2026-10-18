#!/usr/bin/env bash
# ===========================================================================
#  Windows 编译预检（Linux / macOS 版）
#
#  用途：在推 tag 之前，一次性抓出所有 Windows-only 的编译错误，
#        避免等 CI 跑 10 分钟才发现 #[cfg(windows)] 里的问题。
#
#  用法：scripts/check-windows.sh
#
#  前置：已装 Rust、Node、Python。见 docs/CI-WINDOWS-CHECK.md
# ===========================================================================
set -euo pipefail

TARGET="x86_64-pc-windows-msvc"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "============================================================"
echo " WinMacro Windows 编译预检"
echo " target: $TARGET"
echo "============================================================"
echo

# --- 0. 确保 msvc target 存在 ---------------------------------------------
if rustup target list --installed | grep -q "$TARGET"; then
    echo "[0/5] Rust target $TARGET 已安装"
else
    echo "[0/5] 安装 Rust target $TARGET ..."
    rustup target add "$TARGET"
fi
echo

# --- 1. Rust 单元测试（当前平台）-----------------------------------------
echo "[1/5] cargo test - winmacro-logic ..."
cargo test --manifest-path "$ROOT/app-tauri/logic/Cargo.toml"

echo
echo "[2/5] cargo test - recorder_core ..."
cargo test --manifest-path "$ROOT/native/recorder_core/Cargo.toml"
echo

# --- 2. Windows target 编译检查 -------------------------------------------
echo "[3/5] cargo check --target $TARGET - recorder_core ..."
cargo check --target "$TARGET" --all-targets \
    --manifest-path "$ROOT/native/recorder_core/Cargo.toml"

echo
echo "[4/5] cargo check --target $TARGET - src-tauri （较慢，Tauri 依赖树大）..."
cargo check --target "$TARGET" \
    --manifest-path "$ROOT/app-tauri/src-tauri/Cargo.toml"
echo

# --- 3. 前端类型检查 + JS 单测 ---------------------------------------------
echo "[5/5] vue-tsc --noEmit + npm run test:js ..."
cd "$ROOT/app-tauri"
npm install --no-audit --no-fund
npx vue-tsc --noEmit
npm run test:js
echo

echo "============================================================"
echo " 全部通过。可以打 tag 了："
echo "   git tag -a v2.5.0 -m \"...\""
echo "   git push origin v2.5.0"
echo "============================================================"
