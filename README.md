# WinMacro

Windows 脚本制作与执行工具。Python 本地桌面 UI（PySide6，不使用 HTML）+ Rust 高性能键鼠核心。

## 功能

- 添加可拖动的屏幕组件
- 点击组件中的「开始录制」，以整个屏幕为范围记录键盘、鼠标操作
- 点击「结束录制」时，这一次点击不会写入录制内容
- 工具面板中生成新的操作录制；点击后弹出面板，展示每一步的类型、时间、坐标等
- 设置中可自动识别或手动指定录制分辨率，回放时映射到当前屏幕
- 回放支持倍速（0.5x / 1x / 2x / 5x / 立即执行）、循环次数与执行前倒计时

## 安装（推荐）

从 [Releases](https://github.com/plxlaaa/winmacro/releases) 下载最新的 `WinMacro-Setup-x.y.z.exe`，双击运行：

1. 选择安装路径（默认 `C:\Program Files\WinMacro`，可改到任意目录）；
2. 所有文件都会安装在所选目录内，不散落到其他位置；
3. 安装目录中自带 `Uninstall.exe`，**双击它即可打开卸载程序**（也出现在「设置 → 应用」列表中）；
4. 卸载时会询问是否连录制数据（`%USERPROFILE%\.winmacro`）一起删除，默认保留。

安装版已内嵌 Python 运行时与 Rust 原生核心，无需预装任何环境。

## 从源码运行（Windows）

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

推送 `v*` 标签或在 Actions 页手动触发 **Release** 工作流，会在 Windows runner 上：

1. `cargo build --release` 编译 Rust 核心；
2. PyInstaller onedir 打包（版本号取自 `pyproject.toml`，单一来源）；
3. NSIS 生成 `dist/WinMacro-Setup-<版本>.exe`，并上传为 artifact / 附到 Release。

本地验证 NSIS 脚本：`makensis -DVERSION=1.1.0 -DPAYLOAD=<目录> installer/winmacro.nsi`。

## 结构

- `winmacro/` Python 桌面应用
- `native/recorder_core/` Rust 录制 / 回放核心（cdylib）
- `tests/` 录制过滤与数据模型测试

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
