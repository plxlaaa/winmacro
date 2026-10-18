# 贡献指南

感谢对 WinMacro 感兴趣！欢迎提交 Issue 和 Pull Request。

## 提 Issue 前

- **Bug 报告**：写清楚 WinMacro 版本、操作系统版本、能复现的步骤、实际行为与预期行为
- **功能建议**：描述清楚使用场景与价值；如果有 UI 草图或参考项目更好
- **安全相关**：请**不要**在公开 Issue 中披露，请按仓库主页的联系方式私下沟通

## 开发环境

```bash
# 克隆
git clone https://github.com/plxlaaa/winmacro
cd winmacro

# 前端 + Tauri 后端（推荐，新版主线）
cd app-tauri
npm install
npm run tauri dev

# Rust 核心测试（跨平台）
cd ../native/recorder_core && cargo test
cd ../../app-tauri/logic && cargo test

# Python 单元测试（旧版）
cd ../../ && pip install -r requirements.txt pytest && pytest tests/
```

## ⚠️ 推 tag 之前：Windows 编译预检

WinMacro 是 Windows 应用，但 `#[cfg(windows)]` 的代码**在 Linux/macOS 上根本不参与编译**——
`cargo check` 通过 ≠ Windows 能编译。踩过的坑见 [docs/CI-WINDOWS-CHECK.md](docs/CI-WINDOWS-CHECK.md)。

**一条命令跑完全部检查**（Linux/macOS）：

```bash
scripts/check-windows.sh
```

或 Windows 上：

```bat
scripts\check-windows.bat
```

它会依次跑：Rust 单元测试 → `cargo check --target x86_64-pc-windows-msvc`（抓 Windows-only 编译错）
→ `vue-tsc --noEmit` → JS 单测。

**手动等价命令**：

```bash
rustup target add x86_64-pc-windows-msvc
cargo test --manifest-path app-tauri/logic/Cargo.toml
cargo test --manifest-path native/recorder_core/Cargo.toml
cargo check --target x86_64-pc-windows-msvc --manifest-path native/recorder_core/Cargo.toml
cargo check --target x86_64-pc-windows-msvc --manifest-path app-tauri/src-tauri/Cargo.toml
cd app-tauri && npx vue-tsc --noEmit && npm run test:js
```

## 提交规范

格式：`<type>(<scope>): <subject>`

| type | 用途 |
|---|---|
| `feat` | 新功能 |
| `fix` | 修 bug |
| `chore` | 构建/工具/CI |
| `refactor` | 重构（无功能变更） |
| `docs` | 文档 |
| `test` | 测试 |
| `perf` | 性能优化 |

scope 例子：`engine`、`filter`、`model`、`simplify`、`recorder-core`、`tauri`、`vue`...
新功能的第一行 commit message 会被自动收进 CHANGELOG。

## 代码风格

- **Rust**：`cargo fmt` + `cargo clippy` 必须在 CI 通过
- **TypeScript / Vue**：保持现有风格；新增依赖前先讨论
- **Python**：与现有 `winmacro/` 模块风格一致
- 公共 API 改动请同步更新 `README.md` 与 `CHANGELOG.md`

## Pull Request 流程

1. Fork 仓库，新建分支（`feat/xxx`、`fix/xxx`）
2. 在该分支上提交
3. 提交前确认：
   - `cargo test -p winmacro-logic -p recorder_core` 全过
   - `pytest tests/` 全过（如果你动了 Python 端）
   - `cargo fmt --all` 已执行
4. PR 描述写清：
   - 改了什么、为什么改
   - 是否包含破坏性变更
   - 截图（UI 变更时）
5. CI 通过后等 review

## 行为准则

- 善意沟通，对事不对人
- 接受建设性批评
- 关注对项目最有利的事

## License

提交 PR 即表示你同意按 [MIT License](LICENSE) 授权你的贡献。
