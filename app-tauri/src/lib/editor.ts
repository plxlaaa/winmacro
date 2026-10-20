import type { InputEvent } from "../api";

/** Keep the client coordinate in sync so window anchoring respects an edit. */
export function setEventPosition(event: InputEvent, axis: "x" | "y", value: number): boolean {
  if (!Number.isFinite(value)) return false;
  const next = Math.max(0, Math.min(32767, Math.round(value)));
  if (event[axis] === next) return false;
  const localAxis = axis === "x" ? "win_x" : "win_y";
  if (event[axis] != null && event[localAxis] != null) {
    event[localAxis] += next - event[axis];
  }
  event[axis] = next;
  return true;
}
