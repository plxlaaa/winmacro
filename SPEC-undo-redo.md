# SPEC: 脚本编辑 Undo/Redo

> 状态：v2.5 · 让用户敢动大脚本。

## 1. 目标

`StepEditor` 当前 `update_recording_events` 直接覆盖 `rec.events`，没有撤销路径。
大脚本（5000+ 事件）一不小心手滑就是毁灭性删除。

## 2. 设计

纯快照式命令模式：每次破坏性编辑把整个 `events` 数组入栈。
- 50 步 ring buffer
- O(n) per push（5000 事件 ≈ 50 µs，可接受）
- 替换式 (`replace`) 不打断 redo 链，用于连续输入（拖拽等）

## 3. API

`app-tauri/src/lib/undo.ts`：

```ts
class History<T> {
  constructor(initial?: T, opts?: { capacity?: number; clone?: <U>(v: U) => U })
  set(state: T): void            // 初始化 / 重置（不入栈）
  push(state: T): void           // 入栈 + 截断 redo
  replace(state: T): void        // 替换当前（不打断 redo）
  undo(): T | undefined          // 上一状态
  redo(): T | undefined          // 下一状态
  current(): T | undefined
  canUndo(): boolean
  canRedo(): boolean
  subscribe(fn: (s: T, info: { canUndo: boolean; canRedo: boolean }) => void): () => void
  clear(): void
  size(): number                 // 调试用
  position(): number
}
```

## 4. 集成点

| 位置 | 变更 |
|---|---|
| `src/lib/undo.ts` | **新** — 通用 History 类 |
| `src/lib/undo.test.ts` | **新** — 16 个单测（node --test --import tsx） |
| `src/components/StepEditor.vue` | 接入 `History<InputEvent[]>` + `commit()` 在 setGap/setPos/deleteSelected/trimSelected/resetDraft 后调用 + 工具栏 ↶ / ↷ 按钮 + Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y 键绑定 |

## 5. 键绑定

| 键 | 行为 |
|---|---|
| `Ctrl+Z` / `Cmd+Z` | undo |
| `Ctrl+Shift+Z` / `Cmd+Shift+Z` | redo |
| `Ctrl+Y` / `Cmd+Y` | redo (Windows 习惯) |

## 6. 测试

16 个 node:test 单测覆盖：
- 初始状态 / set 种子
- push / undo / redo 基本流
- push after undo 截断 redo
- ring buffer 容量
- capacity=0 无限
- replace 不打断 redo
- clear
- subscribe 接收正确 payload
- 取消订阅
- 默认 clone 防外部突变泄漏
- 自定义 clone 被调用
- undo 到底部 no-op
- redo 到顶部 no-op

## 7. 风险

- **风险 1**：`setGap` / `setPos` 现在每次都 commit，连续拖滑块会疯狂入栈。**已用 number input 的 change 事件触发**，不是 input 事件；按一次回车/失焦才入栈。如果用户觉得太粗，可以改用 `replaceLive` 模式（用 `replace` 不打断 redo 链）。
- **风险 2**：跨多次操作的复合编辑（先 setGap 再 delete）会入栈两次。`Ctrl+Z` 一次撤销一步，符合直觉。
- **回滚**：删 `commit()` 调用 + `history` 变量 = 完整回滚。

## 8. 不在范围

- 跨会话持久化（session-only，关闭弹窗即清）
- 树形历史（分支）
- 多用户协作
