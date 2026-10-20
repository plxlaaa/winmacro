# WinMacro

> **轻量 Windows 宏录制与回放工具** —— Tauri 2 + Vue 3 现代桌面 UI，Rust 高性能键鼠核心。

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Latest Release](https://img.shields.io/github/v/release/plxlaaa/winmacro)](https://github.com/plxlaaa/winmacro/releases/latest)
[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11-0078d4.svg)]()
[![Rust](https://img.shields.io/badge/rust-1.99%2B-orange.svg)]()
[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-24c8db.svg)]()
[![Vue 3](https://img.shields.io/badge/Vue-3.4-42b883.svg)]()

**F10 一键开始录制，再按 F10 结束**——以整个屏幕为范围记录键盘鼠标操作（热键本身不会录进脚本），可绑定全局快捷键回放，倍速、循环、倒计时、隐藏界面全支持。

## 为什么用 WinMacro

- **一键录制**：`F10` 开始 / 结束，`F11` 紧急停止回放
- **零依赖**：单文件 NSIS 安装器（≈ 5 MB），内嵌 WebView2 + Rust 核心
- **安全更新**：minisign 签名校验的自动更新器
- **可移植脚本**：录制库 JSON 格式可导出 / 导入
- **现代 UI**：Tauri 2 + Vue 3，资源占用 < 30 MB

## 功能

- `F10` 全局快捷键开始 / 结束录制，以整个屏幕为范围记录键盘、鼠标操作（热键本身不会录进脚本）
- 录制库管理：按名称或热键搜索、复制脚本、详情逐步查看（类型 / 时间 / 坐标 / 按键）、重命名、删除、导出 / 导入 JSON
- 步骤编辑支持坐标和间隔调整、裁剪、撤销 / 重做；复制的脚本使用独立 ID，并清除热键以避免冲突
- 回放支持倍速（立即 / 0.5× / 1× / 2× / 5×）、循环次数、执行前倒计时；倒计时可通过按钮或 `F11` 取消
- **新设置项**：执行前等待秒数、回放时自动隐藏主界面、是否记录鼠标轨迹（关闭后只录点击 / 滚轮 / 键盘）
- 设置中可自动识别或手动指定录制分辨率，回放时映射到当前屏幕

## 安装（推荐）

从 [Releases](https://github.com/plxlaaa/winmacro/releases) 下载最新的 `WinMacro_x.y.z_x64-setup.exe`，双击运行：

1. 选择安装路径（默认当前用户目录，可改到任意目录）；
2. 所有文件都安装在所选目录内，不散落其他位置；
3. 安装目录自带卸载入口，也出现在「设置 → 应用」列表中；
4. 卸载时可选择是否连录制数据（`%USERPROFILE%\.winmacro`）一并删除。

安装版内嵌完整运行时（WebView2 + Rust 核心），无需预装任何环境。旧版（Python/PySide6）的录制数据与设置自动被 v2.0 沿用。

## 应用内更新（v2.1.0 起）

- 启动 3 秒后自动检查 GitHub Releases（可在设置中关闭），发现新版本弹窗展示版本号与更新说明
- 「立即更新」→ 下载（实时进度条）→ minisign 签名校验 → NSIS 静默安装，全程无需浏览器和 UAC
- 也可在 设置 → 应用更新 中手动检查
- 签名公钥内置在应用内；私钥离线保存，更新包无法被篡改

## 快捷键

- `F10`：开始 / 结束录制
- `F11`：停止回放

## 从源码运行（开发者）

新版（Tauri，主线开发）：

```bash
cd app-tauri
npm install
npm run tauri dev
```

旧版（Python/PySide6，维护模式，源码在 `winmacro/`）：

```bat
python -m pip install -r requirements.txt
python -m winmacro
```

或双击 `scripts\run.bat`。

可选：编译 Rust 核心，获得更低延迟的全局钩子：

```bat
scripts\build_native.bat
```

未编译原生库时，Windows 回退到 Win32 低级钩子；其他系统回退到 pynput。

## 构建安装器（CI 自动完成）

推送 `v*` 标签触发 **Tauri Release** 工作流：Windows runner 上跑前端与 logic/core 单测 → `tauri build` → 签名的 NSIS 中文安装器 → 附到 GitHub Release 草稿。核对构建结果和更新文件后再正式发布。

## 结构

- `app-tauri/` 新版：`logic/`（纯逻辑 crate）+ `src-tauri/`（Tauri 后端）+ `src/`（Vue 3 前端）
- `winmacro/` 旧版 Python 桌面应用（维护模式）
- `native/recorder_core/` Rust 录制 / 回放核心（cdylib + rlib，新旧两版共用）

## 开发与测试

Python 单元测试（无 GUI 依赖，CI 已覆盖）：

```bat
python -m pip install pytest
python -m pytest tests/
```

Rust 核心单元测试（含 Python↔Rust JSON 契约测试，需要 Rust 工具链）：

```bat
cd native\recorder_core
cargo test
```

推送到 GitHub 后，`.github/workflows/ci.yml` 会自动运行 Python、前端、Rust 核心与领域测试，以及前端生产构建；Windows runner 上执行 `cargo check` 验证原生核心和完整 Tauri 后端。
