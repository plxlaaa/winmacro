# SPEC: 回放时间轴 UI

> 状态：v2.5 候选 · 把"数据列表"升级成"可视化时间轴"，让大脚本可读性、可定位性、可监控播放进度都上一个台阶。

## 1. 目标

当前 `DetailsDialog.vue`（500 / 页分页）和 `StepEditor.vue`（300 / 页 + 默认隐藏 move）都是**纯列表**视图。
对 5000+ 事件的脚本：
- 用户不知道"30 秒到 45 秒之间在干嘛"——只能翻页
- 不知道当前播放到哪了——只能等结束
- 编辑时定位"删掉这一段"很难——要肉眼数行号

**本特性**新增一个**Canvas 绘制的密度时间轴**组件，统一嵌入详情与编辑器弹窗顶部，并实时驱动播放头。

## 2. UI 形态

```
┌──────────────────────────────────────────────────────────────┐
│ [ ▶ 立即 ] [ 0.5× ] [ 1× ] [ 2× ] [ 5× ]   1234 / 5000 (25%)  │
├──────────────────────────────────────────────────────────────┤
│ ▁▁▂▂▆█▆▂▁▁ ▁▁▁▁▁▂▃▅▆▅▃▁ ▁▁▁▂▃▅▇▆▄▂▁ ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁ │  ← 移动（蓝）
│            ▁▁ ▁         ▁▁ ▁    ▁▁ ▁         ▁             │  ← 点击（红）
│              ▁           ▁       ▁                          │  ← 滚轮（紫）
│                  ▁▁▁▁▁▁▁▁▁     ▁▁▁▁▁▁▁▁▁▁                  │  ← 键盘（绿）
│                       │                                       │  ← 播放头（橙）
└──────────────────────────────────────────────────────────────┘
       0s              15s              30s              45s
```

- 4 泳道（移动 / 点击 / 滚轮 / 键盘），颜色与 app 主题一致
- 鼠标悬停 → tooltip 显示该列 t_ms 与各类型计数
- 点击 → emit `seek(tMs)` 给父组件，父组件翻到对应页 + 滚动到行
- 播放期间 → 橙色播放头横向移动（实时跟随 `play:progress` 事件）
- 不可播放或空录制 → 显示"空录制"占位

## 3. 组件 API

`app-tauri/src/components/Timeline.vue`：

```ts
defineProps<{
  events: InputEvent[];        // 必须
  currentMs?: number | null;   // 播放头位置（毫秒）；null/undefined 时不画
  durationMs?: number;         // 总时长；不传则取 events 最后一项 t_ms
  height?: number;             // canvas 高度，默认 60
}>()

defineEmits<{
  (e: "seek", tMs: number): void;  // 点击时间轴
}>()
```

## 4. 后端事件

新增 `play:progress` 事件，由 `recorder_core` 的播放循环触发：

```json
{ "index": 1234, "total": 5000 }
```

- 每发完一个事件 → C ABI 回调 `progress_trampoline(index, total)` 被调用
- 节流：距上次 emit ≥ 60ms 才发；最后一次（`index == total`）必发
- 停止播放 / 异常结束 → 调 `recorder_set_progress_cb(None)` 清掉

`recorder_core` 新增 C ABI：

```rust
type ProgressCb = unsafe extern "C" fn(index: u32, total: u32);
pub fn recorder_set_progress_cb(cb: Option<ProgressCb>);
```

`AppState` 新增 `last_progress_ms: AtomicI64`（节流时间戳）。
`engine.rs` 新增 `install_progress_callback(&AppHandle)`（幂等）。

## 5. 集成点

| 位置 | 变更 |
|---|---|
| `native/recorder_core/src/lib.rs` | 加 `PROGRESS_CB` 静态槽 + `recorder_set_progress_cb` 导出 + `playback` 循环里 emit + 单测 |
| `app-tauri/src-tauri/src/engine.rs` | `AppState` 加 `last_progress_ms`；加 `progress_trampoline` + `install_progress_callback`；`play_recording` 装回调、`finish` 清回调 |
| `app-tauri/src/components/Timeline.vue` | **新文件**——Canvas 组件 |
| `app-tauri/src/components/DetailsDialog.vue` | 顶部嵌入 Timeline + `setCurrentMs` 方法 + `seek` 函数 |
| `app-tauri/src/components/StepEditor.vue` | 顶部嵌入 Timeline（48 px 高）；`seek` 二分定位 + `scrollIntoView` |
| `app-tauri/src/views/Library.vue` | 监听 `play:progress`，推给详情弹窗的 `setCurrentMs`；状态栏加「已播 1234 / 5000 (25%)」 |

## 6. 性能预算

- 5 万事件的录制：
  - Canvas 绘制：4 泳道 × 600 px 宽 → 2,400 个桶 → < 1ms 一次 draw
  - `rebuildBuckets` 每次 5 万次循环 + 4 次查找 → O(n) ≈ 5ms（每 60ms 才重画一次，OK）
  - 悬停 tooltip：O(1) 桶查询
- 内存：4 × Uint32Array(300) ≈ 5 KB，常驻

## 7. 测试

`recorder_core`：
- ✅ progress callback 设置 / 清除（裸 `emit_progress` 验证）
- 旧测试不受影响（Python 端不调 `recorder_set_progress_cb`）

未来在 Windows runner 上可加：
- 真实播放 100 个事件 → 验证 callback 被调用 100 次 + index 是 1..100
- 60ms 节流在跨进程下工作正常

## 8. 风险与回滚

- **风险 1**：`progress_trampoline` 在 recorder_core 的播放线程跑，调用 Tauri 的 `app.emit` 跨线程。Tauri 2 的 `AppHandle::emit` 是 thread-safe（内部用 mpsc），但要小心 `app.state::<T>()` 在非 Tauri 管理线程的语义。我们的 `AppState` 全是 `Send + Sync` 字段，OK。
- **风险 2**：JS 端不刷新 `currentMs` 时，详情弹窗开着但不放 → 状态会卡在最后一次 emit 的 ms。**已处理**：`play:done` / `play:stopped` / `play:error` 都会清空 `currentMs`。
- **回滚**：仅影响 Tauri 后端 + Vue 前端；Python 旧版无 progress 概念，无破坏。把 `progress_trampoline` 和 `install_progress_callback` 删掉即可，UI 端不引入 Timeline 组件就完全回滚。

## 9. 不在本次范围

- 多泳道分别缩放（现在是按每条泳道自身最大值归一化，跨泳道不可比）
- 缩放/平移（zoom & pan）—— 60px 高的密度图对 30 秒内录制足够，再长就改 #1 窗口锚定
- 缩略图预览（鼠标位置小图叠加在泳道上）—— 见 #1
- 实时录制时的"草稿时间轴"—— 当前 rec:progress 是事件计数，不是时间维度
