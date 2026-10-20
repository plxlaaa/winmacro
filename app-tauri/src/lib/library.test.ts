import { test } from "node:test";
import assert from "node:assert/strict";
import { filterRecordings } from "./library";
import type { Recording } from "../api";

const recordings: Recording[] = [
  { id: "a", name: "每日 报表", hotkey: "Control+Alt+1" },
  { id: "b", name: "Browser Setup", hotkey: null },
] as Recording[];

test("blank search preserves library order", () => {
  assert.deepEqual(filterRecordings(recordings, "  "), recordings);
});
test("search matches Chinese, case-insensitive names and Ctrl hotkeys", () => {
  assert.equal(filterRecordings(recordings, "报表 ctrl+alt")[0]?.id, "a");
  assert.equal(filterRecordings(recordings, " BROWSER ")[0]?.id, "b");
});
test("all terms must match without modifying recordings", () => {
  assert.deepEqual(filterRecordings(recordings, "报表 browser"), []);
  assert.equal(recordings.length, 2);
  assert.equal(recordings[0].hotkey, "Control+Alt+1");
});
