# SPEC: 窗口锚定回放

> 状态：v2.5 · 解决"换屏/换分辨率/窗口移动就翻车"的核心痛点。

## 1. 目标

当前 `transform::remap_events` 把所有坐标当成"整屏归一化"线性缩放。
1920×1080 录的脚本到 2560×1440 还行，**但窗口位置变了**就完全错位——

```
录的时候 Chrome 在屏幕左上角 (0,0)，按钮在 (200, 50)
回放时 Chrome 在屏幕中间 (960, 540) ——
   旧逻辑会点屏幕 (200, 50)，完全错过按钮
```

## 2. 方案：双坐标系

录制时给每个 mouse 事件加 3 个字段：
- `target_hwnd: u32` — 该事件命中的前台窗口（Win32 HWND）
- `win_x: i32` — 该事件在窗口**客户区**的 x
- `win_y: i32` — 该事件在窗口**客户区**的 y

回放时：
1. 用 `target_hwnd` 找窗口 → `GetWindowRect` 得当前屏幕位置 (left, top, w, h)
2. 新屏幕坐标 = (left + win_x, top + win_y)
3. 找不到窗口 → 回退到全屏缩放

**结果**：脚本跟着窗口走。多屏切了、笔记本拔了 HDMI、Chrome 从左拖到右、分辨率从 1080p 升到 4K —— 全部不翻车。

## 3. 录制侧

`recorder_core/src/lib.rs::win_impl::mouse_proc` 在每个 mouse 事件时调 `capture_anchor(x, y)`：
- `GetForegroundWindow()` → HWND（无则 None）
- `ScreenToClient(hwnd, &mut POINT)` → 客户区坐标
- 失败时 win_x/win_y = None（target_hwnd 仍记）

开销：每次鼠标事件 +2 个 Win32 调用（~1 µs），在 8ms 节流之后几乎无感。

## 4. 回放侧

`winmacro_logic::transform::remap_to_anchor` —— 纯函数，**不依赖平台 API**：

```rust
pub struct WindowBounds { x: i32, y: i32, w: i32, h: i32 }

pub fn remap_to_anchor<F>(
    events: &[InputEvent],
    fw: i32, fh: i32,  // 录制时屏幕
    tw: i32, th: i32,  // 当前屏幕
    use_anchor: bool,
    anchor_lookup: F,  // hwnd -> Option<WindowBounds>
) -> Vec<InputEvent>
where F: FnMut(u32) -> Option<WindowBounds>
```

测试可注入 mock closure（7 个单测覆盖各种回退路径）。

`engine.rs::play_recording` 调 `remap_to_anchor` 替代 `remap_events`，传入 `lookup_window_bounds`（Windows 用 `GetWindowRect`，非 Windows 永远 None）。

## 5. 设置

`Settings.use_window_anchor: bool`，默认 `true`。关闭后用 v2.4 的全屏缩放（适合无边框全屏游戏、录制时屏幕与回放时一致的简单场景）。

## 6. 兼容性

- **旧 recordings**：无 `target_hwnd` 字段 → `remap_to_anchor` 走全屏缩放路径，行为与 v2.4 一致
- **新 recordings**：有字段 → 优先按窗口定位
- **JSON / bincode**：`#[serde(default)]` 保兼容，老文件加载正常

## 7. 集成点

| 位置 | 变更 |
|---|---|
| `logic/src/model.rs::InputEvent` | + `target_hwnd` / `win_x` / `win_y` 可选字段 |
| `logic/src/model.rs::Settings` | + `use_window_anchor: bool` |
| `logic/src/transform.rs` | + `WindowBounds` 结构 + `remap_to_anchor` 函数 + 7 个单测 |
| `native/recorder_core/src/lib.rs` | + `capture_anchor` helper + 字段填充（Windows only） |
| `src-tauri/src/engine.rs` | + `lookup_window_bounds` 平台分支 + 调 `remap_to_anchor` |
| `app-tauri/src/api.ts` | Settings 加 `use_window_anchor` |
| `app-tauri/src/views/Settings.vue` | 加「窗口锚定回放」开关 |

## 8. 测试

`transform::tests`：7 个新单测
- ✅ 命中窗口：bounds + win_x/y → 屏幕坐标
- ✅ 窗口找不到：回退全屏缩放
- ✅ 无 hwnd 记录（旧数据）：回退全屏缩放
- ✅ use_anchor = false：不调 lookup，直接全屏缩放
- ✅ win_x 缺：回退全屏缩放
- ✅ 键盘事件：不动
- ✅ `WindowBounds::to_screen` 基础计算

Win32 部分（`capture_anchor` / `lookup_window_bounds`）只在 Windows 编译，需在 Windows runner 上冒烟测试。

## 9. 风险

- **风险 1**：窗口 Z 序变化。用户切换了窗口（点开其它程序再切回来），前台窗口变了，`target_hwnd` 也跟着变。**这是预期行为**，但需要在 UX 上让用户知道"脚本只对当时那个窗口有效"——已加在设置说明里。
- **风险 2**：管理员/服务窗口。某些窗口 `ScreenToClient` 失败 → win_x/win_y = None → 回退全屏缩放。**已处理**。
- **风险 3**：跨显示器负坐标。Windows 多屏下主屏不一定在 (0,0)，`GetWindowRect` 可能返回负的 left/top。`to_screen` 直接相加，结果正确。
- **回滚**：`Settings.use_window_anchor = false` 即回退到全屏缩放；删字段即回滚到 v2.4 行为。

## 10. 不在范围

- 图像识别兜底（万一窗口换了标题栏文字但位置/大小一致）
- DPI 变化适配（同一窗口移动到不同 DPI 屏）
- 子控件级别定位（识别"点的是按钮 B 而不是按钮 A"）
- 跨进程窗口（UAC 提升的窗口无法注入）
