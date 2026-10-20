/**
 * 轻量级撤销/重做历史栈：基于快照的命令模式。
 *
 * 用法：
 * ```ts
 * const hist = new History<string[]>([], { capacity: 50 });
 * hist.set(["a", "b", "c"]);     // 设为初始状态
 * hist.push(["a", "c"]);         // 提交变更（保存快照）
 * hist.undo();                   // ["a", "b", "c"]
 * hist.redo();                   // ["a", "c"]
 * ```
 *
 * 设计要点：
 * - `set(state)` 用于初始化或重置，不入栈（避免污染历史）
 * - `push(state)` 入栈；任何"未来"（被 undo 后再 push）会被丢弃
 * - `replace(state)` 入栈但**不**打断 redo 链（用于纯几何调整如 drag 缩放）
 * - ring buffer 满后丢最早的
 * - 提供 `subscribe` 让 Vue 直接 `watchEffect` 同步
 *
 * 快照读写均深拷贝，避免编辑 Vue 响应式草稿时污染历史。
 */

export interface HistoryOptions {
  /** 最大历史深度（默认 50，0 = 无限） */
  capacity?: number;
  /** 自定义快照拷贝；必须返回与输入独立的值。 */
  clone?: <T>(v: T) => T;
}

export type Listener<T> = (state: T, info: { canUndo: boolean; canRedo: boolean }) => void;

const defaultClone = <T>(v: T): T => {
  // 递归访问也支持 Vue Proxy；structuredClone 无法克隆响应式代理。
  if (Array.isArray(v)) return v.map((item) => defaultClone(item)) as T;
  if (v && typeof v === "object") {
    return Object.fromEntries(
      Object.entries(v).map(([key, value]) => [key, defaultClone(value)]),
    ) as T;
  }
  return v;
};

export class History<T> {
  private stack: T[] = [];
  private index = -1; // 指向当前状态在 stack 中的下标；-1 = 未初始化
  private readonly capacity: number;
  private readonly clone: <U>(v: U) => U;
  private listeners = new Set<Listener<T>>();

  constructor(initial?: T, opts: HistoryOptions = {}) {
    this.capacity = Math.max(0, opts.capacity ?? 50);
    this.clone = (opts.clone ?? defaultClone) as <U>(v: U) => U;
    if (initial !== undefined) {
      this.stack.push(this.clone(initial));
      this.index = 0;
    }
  }

  /** 当前状态（未初始化时返回 undefined） */
  current(): T | undefined {
    return this.index >= 0 ? this.clone(this.stack[this.index]) : undefined;
  }

  canUndo(): boolean {
    return this.index > 0;
  }

  canRedo(): boolean {
    return this.index >= 0 && this.index < this.stack.length - 1;
  }

  /** 初始化或重置历史。**不**入栈（避免首屏 undo 直接回到 undefined）。 */
  set(state: T): void {
    this.stack = [this.clone(state)];
    this.index = 0;
    this.notify();
  }

  /** 提交一个新状态。丢弃当前 index 之后的所有 redo。 */
  push(state: T): void {
    if (this.index < 0) {
      this.set(state);
      return;
    }
    // 截断 redo
    if (this.index < this.stack.length - 1) {
      this.stack.length = this.index + 1;
    }
    this.stack.push(this.clone(state));
    // ring buffer
    if (this.capacity > 0 && this.stack.length > this.capacity) {
      const overflow = this.stack.length - this.capacity;
      this.stack.splice(0, overflow);
    }
    this.index = this.stack.length - 1;
    this.notify();
  }

  /**
   * 替换当前状态但**不**打断 redo 链。适用于纯几何调整（拖拽时）
   * 或可被等效 redo 覆盖的连续输入。
   */
  replace(state: T): void {
    if (this.index < 0) {
      this.set(state);
      return;
    }
    this.stack[this.index] = this.clone(state);
    this.notify();
  }

  undo(): T | undefined {
    if (!this.canUndo()) return undefined;
    this.index -= 1;
    this.notify();
    return this.current();
  }

  redo(): T | undefined {
    if (!this.canRedo()) return undefined;
    this.index += 1;
    this.notify();
    return this.current();
  }

  /** 清空（用于关闭弹窗等）。 */
  clear(): void {
    this.stack = [];
    this.index = -1;
    this.notify();
  }

  /** 历史长度（含当前），调试用。 */
  size(): number {
    return this.stack.length;
  }

  /** 当前 index（0 = 最老），调试用。 */
  position(): number {
    return this.index;
  }

  subscribe(fn: Listener<T>): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  private notify(): void {
    if (this.listeners.size === 0) return;
    const cur = this.index >= 0 ? this.stack[this.index] : undefined;
    if (cur === undefined) return;
    const info = { canUndo: this.canUndo(), canRedo: this.canRedo() };
    for (const fn of this.listeners) fn(this.clone(cur), { ...info });
  }
}
