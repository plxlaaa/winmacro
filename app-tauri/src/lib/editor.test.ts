import { test } from "node:test";
import assert from "node:assert/strict";
import { setEventPosition } from "./editor";

test("coordinate edits preserve the client-to-screen offset", () => {
  const event = { kind: "mouse_down", t_ms: 0, x: 300, y: 200, target_hwnd: 1, win_x: 50, win_y: 25 };
  assert.equal(setEventPosition(event, "x", 330), true);
  assert.equal(event.win_x, 80);
  assert.equal(event.x - event.win_x, 250);
  assert.equal(event.win_y, 25);
});
test("legacy coordinates can be edited without adding anchor data", () => {
  const event = { kind: "mouse_move", t_ms: 0, x: 4, y: 5 };
  assert.equal(setEventPosition(event, "y", 12.6), true);
  assert.equal(event.y, 13);
  assert.equal("win_y" in event, false);
});
test("invalid and unchanged values do not create edits", () => {
  const event = { kind: "mouse_move", t_ms: 0, x: 4 };
  assert.equal(setEventPosition(event, "x", NaN), false);
  assert.equal(setEventPosition(event, "x", Infinity), false);
  assert.equal(setEventPosition(event, "x", 4), false);
  assert.equal(event.x, 4);
});
