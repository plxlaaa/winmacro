/**
 * 用 node 内置 test runner 跑（`node --test --import tsx`）。
 * 验证 History 的状态机正确性。
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { History } from "./undo";
import { reactive } from "vue";

test("editing nested reactive events cannot mutate earlier snapshots", () => {
  const events = reactive([{ t_ms: 10, position: { x: 1 } }]);
  const h = new History(events);
  events[0].t_ms = 20;
  events[0].position.x = 2;
  h.push(events);
  assert.deepEqual(h.undo(), [{ t_ms: 10, position: { x: 1 } }]);
  assert.deepEqual(h.redo(), [{ t_ms: 20, position: { x: 2 } }]);
});

test("editing after undo creates an independent branch", () => {
  const h = new History([{ x: 1 }]);
  h.push([{ x: 2 }]);
  const draft = h.undo()!;
  draft[0].x = 3;
  assert.deepEqual(h.current(), [{ x: 1 }]);
  h.push(draft);
  assert.equal(h.canRedo(), false);
  assert.deepEqual(h.undo(), [{ x: 1 }]);
});

test("readers and subscribers receive isolated copies", () => {
  const h = new History([{ x: 1 }]);
  let seen = 0;
  h.subscribe((draft) => { draft[0].x = 999; });
  h.subscribe((draft) => { seen = draft[0].x; });
  h.push([{ x: 2 }]);
  h.current()![0].x = 888;
  assert.equal(seen, 2);
  assert.deepEqual(h.current(), [{ x: 2 }]);
});

test("initial state with no value", () => {
  const h = new History<number[]>();
  assert.equal(h.current(), undefined);
  assert.equal(h.canUndo(), false);
  assert.equal(h.canRedo(), false);
});

test("set seeds the history", () => {
  const h = new History<number[]>([1, 2, 3]);
  assert.deepEqual(h.current(), [1, 2, 3]);
  assert.equal(h.canUndo(), false);
  assert.equal(h.canRedo(), false);
  assert.equal(h.size(), 1);
  assert.equal(h.position(), 0);
});

test("push then undo returns to previous", () => {
  const h = new History<string[]>(["a"]);
  h.push(["a", "b"]);
  h.push(["a", "b", "c"]);
  assert.deepEqual(h.current(), ["a", "b", "c"]);

  assert.deepEqual(h.undo(), ["a", "b"]);
  assert.deepEqual(h.undo(), ["a"]);
  assert.equal(h.canUndo(), false);
  assert.equal(h.canRedo(), true);
});

test("redo after undo restores future", () => {
  const h = new History<number[]>([1]);
  h.push([2]);
  h.push([3]);
  assert.deepEqual(h.undo(), [2]);
  assert.deepEqual(h.undo(), [1]);
  assert.deepEqual(h.redo(), [2]);
  assert.deepEqual(h.redo(), [3]);
});

test("push after undo drops the redo branch", () => {
  const h = new History<string[]>(["a"]);
  h.push(["b"]);
  h.push(["c"]);
  h.undo(); // back to b
  h.push(["d"]); // drops c
  assert.deepEqual(h.current(), ["d"]);
  assert.equal(h.canRedo(), false);
  assert.equal(h.size(), 3); // a, b, d
  h.undo();
  assert.deepEqual(h.current(), ["b"]);
});

test("ring buffer drops oldest when over capacity", () => {
  const h = new History<number>(0, { capacity: 3 });
  h.push(1);
  h.push(2);
  h.push(3);
  h.push(4); // over capacity
  h.push(5); // over capacity
  assert.equal(h.size(), 3);
  assert.equal(h.current(), 5);
  assert.deepEqual(h.undo(), 4);
  assert.deepEqual(h.undo(), 3);
  assert.equal(h.canUndo(), false);
});

test("capacity = 0 means unlimited", () => {
  const h = new History<number>(0, { capacity: 0 });
  for (let i = 1; i <= 1000; i++) h.push(i);
  assert.equal(h.size(), 1001);
  assert.equal(h.current(), 1000);
});

test("replace updates current without breaking redo chain", () => {
  const h = new History<string[]>(["a"]);
  h.push(["b"]);
  h.push(["c"]);
  h.undo(); // back to b
  h.replace(["b*"]); // mutate b, but keep redo
  assert.deepEqual(h.current(), ["b*"]);
  assert.equal(h.canRedo(), true);
  assert.deepEqual(h.redo(), ["c"]);
});

test("replace on empty history behaves like set", () => {
  const h = new History<string[]>();
  h.replace(["x"]);
  assert.deepEqual(h.current(), ["x"]);
  assert.equal(h.size(), 1);
});

test("clear wipes history", () => {
  const h = new History<number>(0);
  h.push(1);
  h.push(2);
  h.clear();
  assert.equal(h.current(), undefined);
  assert.equal(h.size(), 0);
  assert.equal(h.canUndo(), false);
  assert.equal(h.canRedo(), false);
});

test("subscriber receives current state and can* flags", () => {
  const h = new History<number[]>([1]);
  const seen: Array<{ state: number[]; canUndo: boolean; canRedo: boolean }> = [];
  const unsub = h.subscribe((s, i) => seen.push({ state: [...s], ...i }));

  h.push([1, 2]);
  h.push([1, 2, 3]);
  h.undo();
  h.undo();
  h.redo();

  unsub();

  // 注意：构造器不触发 notify。第一次 notify 是首次 push。
  // expected transitions:
  //   push [1,2]                      → canUndo=true,  canRedo=false
  //   push [1,2,3]                    → canUndo=true,  canRedo=false
  //   undo → [1,2]                    → canUndo=true,  canRedo=true
  //   undo → [1]                      → canUndo=false, canRedo=true
  //   redo → [1,2]                    → canUndo=true,  canRedo=true
  assert.equal(seen.length, 5);
  assert.deepEqual(seen[0], { state: [1, 2], canUndo: true, canRedo: false });
  assert.deepEqual(seen[1], { state: [1, 2, 3], canUndo: true, canRedo: false });
  assert.deepEqual(seen[2], { state: [1, 2], canUndo: true, canRedo: true });
  assert.deepEqual(seen[3], { state: [1], canUndo: false, canRedo: true });
  assert.deepEqual(seen[4], { state: [1, 2], canUndo: true, canRedo: true });
});

test("subscriber can be unsubscribed mid-flight", () => {
  const h = new History<number>(0);
  let count = 0;
  const unsub = h.subscribe(() => count++);
  h.push(1);
  h.push(2);
  assert.equal(count, 2);
  unsub();
  h.push(3);
  assert.equal(count, 2);
});

test("external array mutations do not leak into history", () => {
  const original = [{ id: 1 }, { id: 2 }];
  const h = new History(original);
  // mutate the externally-held array
  original.push({ id: 3 });
  // history's current state should still be the original snapshot
  assert.equal(h.current()!.length, 2);
});

test("custom clone is honored", () => {
  let calls = 0;
  const h = new History<number>(0, {
    clone: (v) => {
      calls++;
      return v;
    },
  });
  h.push(1);
  h.push(2);
  // 1 for set + 2 for push
  assert.equal(calls, 3);
});

test("undo at the bottom is a no-op and returns undefined", () => {
  const h = new History<string[]>(["a"]);
  const result = h.undo();
  assert.equal(result, undefined);
  assert.deepEqual(h.current(), ["a"]);
  assert.equal(h.canUndo(), false);
});

test("redo past the top is a no-op", () => {
  const h = new History<string[]>(["a"]);
  h.push(["b"]);
  const result = h.redo();
  assert.equal(result, undefined);
  assert.deepEqual(h.current(), ["b"]);
});
