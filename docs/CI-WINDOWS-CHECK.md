# 在 Linux 上验证 Windows 编译

> WinMacro 是 Windows 应用，但开发环境可能是 Linux / macOS。
> 这份文档说明**如何在非 Windows 机器上提前发现 Windows-only 的编译错误**，
> 而不是等 CI 跑 10 分钟后才发现。

## 0. 为什么需要这个

Rust 的 `#[cfg(windows)]` 代码块**在 Linux 上根本不参与编译**：

```bash
# Linux target — 这段代码被 cfg 掉，永远不会被类型检查
cargo check
#   → ✅ 通过，但 Windows 专属代码一个字都没检查
```

典型翻车现场：CI 报错 → 修 → 再推 → 又报错。每轮 10 分钟。

本项目 v2.5.0 发版时连续踩了 **4 个** 只有 Windows 才会暴露的坑：

| 轮次 | 错误 | 只有 Windows 才暴露的原因 |
|---|---|---|
| 1 | `unresolved import ...::ScreenToClient` | `#[cfg(windows)] mod win_impl` 整个模块 |
| 2 | `cannot find Graphics in Win32` | 同上（feature 门控） |
| 3 | `no method named is_ok found for struct BOOL` | 同上（返回类型差异） |
| 4 | `Cannot find module 'node:test'` | `vue-tsc` 检查 `src/` 全量 |

**第 3 轮时还在本地"验过了"**——因为 Linux 编译根本不碰那段代码。

## 1. 一条命令搞定

```bash
# 装 msvc target（只需一次）
rustup target add x86_64-pc-windows-msvc

# 检查 Windows 编译（不链接，几秒出结果）
cargo check --target x86_64-pc-windows-msvc
```

`cargo check` **不做链接**，所以 Linux 上不需要 MSVC 链接器。

### 本项目的两个 crate

```bash
# 核心库（快，1-2 秒）
cargo check --target x86_64-pc-windows-msvc --manifest-path native/recorder_core/Cargo.toml

# Tauri 后端（慢，Tauri 依赖树巨大，第一次要十几分钟）
cargo check --target x86_64-pc-windows-msvc --manifest-path app-tauri/src-tauri/Cargo.toml
```

## 2. 加速：配置国内镜像

crates.io 直连在很多网络环境下极慢。写 `~/.cargo/config.toml`：

```toml
[source.crates-io]
replace-with = 'rsproxy-sparse'

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[net]
git-fetch-with-cli = true
retry = 5
```

rustup 本身也走镜像：

```bash
export RUSTUP_DIST_SERVER=https://rsproxy.cn
export RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup
```

## 3. 前端类型检查（独立于 cargo）

`tauri build` 会先跑 `beforeBuildCommand`（本项目是 `npm run build` = `vue-tsc --noEmit && vite build`）。
这一关也会挂，而且跟 Rust 无关：

```bash
cd app-tauri
npm install          # 第一次
npx vue-tsc --noEmit  # 单独跑类型检查
```

### 测试文件要用 node: 内建模块时

`src/**/*.test.ts` 里 `import { test } from "node:test"` 需要 `@types/node`。
两个选择：

**A. 装 `@types/node`**（类型完整，但多一个 devDependency）

```bash
npm i -D @types/node
```

**B. 从类型检查里排除测试文件**（本项目采用，零依赖）

```jsonc
// app-tauri/tsconfig.json
{
  "include": ["src"],
  "exclude": ["src/**/*.test.ts", "src/**/*.spec.ts"]
}
```

排除后测试照常跑（`npm run test:js` 用 `tsx --test`，不读 tsconfig 的 include/exclude），
但测试文件本身不再享受 `vue-tsc` 的类型检查。

## 4. `windows` crate 的两个坑

WinMacro 锁在 `windows = "0.58"`（跟 Tauri 2 兼容）。

### 坑 1：feature 是层级的，子 feature 不传递到父

`windows` crate 把 API 按命名空间切成 feature。**用子命名空间必须同时开父命名空间**：

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    # ... 其它
    "Win32_Graphics",       # 父 — 启用 `pub mod Graphics;`
    "Win32_Graphics_Gdi",   # 子 — 启用 `pub mod Gdi;`（含 ScreenToClient）
] }
```

只开 `Win32_Graphics_Gdi` 会报：

```
error[E0433]: cannot find `Graphics` in `Win32`
note: the item is gated behind the `Win32_Graphics` feature
```

**规律**：`Win32_A_B_C` 用的时候，`Win32_A_B` 和 `Win32_A` 都要开。

### 坑 2：0.58 里 BOOL 和 Result 混用

同一个 crate 里，不同 API 的返回类型**不统一**：

| API | 0.58 返回 | 0.59+ 返回 |
|---|---|---|
| `ScreenToClient` | `BOOL`（i32 newtype） | `Result<(), Error>` |
| `GetWindowRect` | `BOOL` | `Result<(), Error>` |
| `SetProcessDpiAwareness` | `Result<(), Error>` | `Result<(), Error>` |

`BOOL` 没有 `.is_ok()`，`Result` 没有 `.0`。**不能统一改写调用点**。

本项目的解法：`recorder_core::WinOk` trait，对两种类型都实现：

```rust
// native/recorder_core/src/lib.rs
pub trait WinOk {
    fn win_ok(&self) -> bool;
}

impl WinOk for windows::Win32::Foundation::BOOL {
    fn win_ok(&self) -> bool { self.0 != 0 }
}

// 泛型 blanket impl：不用指名 Error 类型（0.58 / 0.59 路径不同）
impl<E> WinOk for Result<(), E> {
    fn win_ok(&self) -> bool { self.is_ok() }
}
```

调用点统一写 `.win_ok()`：

```rust
if ScreenToClient(hwnd, &mut pt).win_ok() { ... }
if SetProcessDpiAwareness(PROCESS_SYSTEM_DPI_AWARE).win_ok() { ... }
```

**这样 crate 从 0.58 升到 0.59+ 时不会因为返回类型变更炸。**
`src-tauri` 通过 `use recorder_core::WinOk;` 复用同一个 trait。

## 5. 本地 vs CI 能力对照

| 检查项 | 本地 Linux | CI (windows-latest) |
|---|---|---|
| Rust Linux target 编译 | ✅ 秒级 | ✅ |
| Rust Windows target 编译 | ✅ 需 msvc target | ✅ |
| Rust 单元测试（Linux） | ✅ | ✅ |
| Rust 单元测试（Windows） | ❌ 跑不了（需 Windows 运行时） | ✅ |
| Vue/TS 类型检查 | ✅ `npx vue-tsc --noEmit` | ✅ |
| Tauri 完整 build | ❌ 需 Windows SDK | ✅ |
| NSIS 打包 | ❌ | ✅ |
| 端到端手测 | ❌ | ❌（还是要你自己装） |

**结论**：本地能抓走 CI 80% 的失败（编译期），剩下的是打包和运行时问题。

## 6. 建议的提交前检查

```bash
# 1. Rust 全量测试（Linux，最快）
cargo test --manifest-path app-tauri/logic/Cargo.toml
cargo test --manifest-path native/recorder_core/Cargo.toml

# 2. Windows target 编译检查（抓 cfg(windows) 的错）
cargo check --target x86_64-pc-windows-msvc --manifest-path native/recorder_core/Cargo.toml
cargo check --target x86_64-pc-windows-msvc --manifest-path app-tauri/src-tauri/Cargo.toml

# 3. 前端类型检查 + 单测
cd app-tauri
npx vue-tsc --noEmit
npm run test:js

# 4. Python 端
pip install -r requirements.txt pytest && pytest tests/
```

**只做 1+2 就能覆盖 v2.5.0 那 4 轮失败中的前 3 轮**（第 4 轮是 TS，步骤 3 覆盖）。

## 7. 打 tag 时的注意事项

本项目 `tauri.yml` 在 `push: tags: ["v*"]` 时触发完整构建。
**tag 指向的 commit 决定了构建内容**——如果 tag 打早了、之后再修代码，
必须**重新打 tag**：

```bash
git tag -d v2.5.0
git tag -a v2.5.0 <new-commit> -m "..."
git push origin :refs/tags/v2.5.0     # 删远端
git push origin v2.5.0                # 推新的
```

注意：**push 同一个 tag 名到新 commit 可能不会重新触发 workflow**。
如果 CI 没跑，用 `workflow_dispatch` 手动触发：

```bash
curl -X POST -H "Authorization: token $TOKEN" \
  -H "Accept: application/vnd.github+json" \
  https://api.github.com/repos/plxlaaa/winmacro/actions/workflows/tauri.yml/dispatches \
  -d '{"ref":"master"}'
```

## 相关

- `CHANGELOG.md` — 版本变更记录
- `.github/workflows/ci.yml` — CI 定义
- `.github/workflows/tauri.yml` — 发版构建
