# WinMacro

Windows 脚本制作与执行工具。**Tauri 2 + Vue 3** 现代桌面 UI + Rust 高性能键鼠核心（v2.0 起重写，跨平台 UI 框架）。

## 功能

- `F10` 全局快捷键开始 / 结束录制，以整个屏幕为范围记录键盘、鼠标操作（热键本身不会录进脚本）
- 录制库管理：详情逐步查看（类型 / 时间 / 坐标 / 按键）、重命名、删除、导出 / 导入 JSON
- 回放支持倍速（立即 / 0.5× / 1× / 2× / 5×）、循环次数、执行前倒计时
- **新设置项**：执行前等待秒数、回放时自动隐藏主界面、是否记录鼠标轨迹（关闭后只录点击 / 滚轮 / 键盘）
- 设置中可自动识别或手动指定录制分辨率，回放时映射到当前屏幕

## 安装（推荐）

从 [Releases](https://github.com/plxlaaa/winmacro/releases) 下载最新的 `WinMacro_x.y.z_x64-setup.exe`，双击运行：

1. 选择安装路径（默认当前用户目录，可改到任意目录）；
2. 所有文件都安装在所选目录内，不散落其他位置；
3. 安装目录自带卸载入口，也出现在「设置 → 应用」列表中；
4. 卸载时可选择是否连录制数据（`%USERPROFILE%\.winmacro`）一并删除。

安装版内嵌完整运行时（WebView2 + Rust 核心），无需预装任何环境。旧版（Python/PySide6）的录制数据与设置自动被 v2.0 沿用。

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

推送 `v*` 标签触发 **Tauri Release** 工作流：Windows runner 上跑 logic/core 单测 → `tauri build` → NSIS 中文安装器 → 附到 GitHub Release。

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

推送到 GitHub 后，`.github/workflows/ci.yml` 会自动运行上述测试，并在 Windows 上执行 `cargo check` 验证原生核心。
