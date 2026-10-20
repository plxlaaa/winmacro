import type { Recording } from "../api";

/** Match all search terms against the name or the familiar Ctrl hotkey label. */
export function filterRecordings(recordings: Recording[], query: string): Recording[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return recordings.filter((rec) => {
    const text = `${rec.name} ${rec.hotkey ?? ""} ${(rec.hotkey ?? "").replace(/Control/gi, "Ctrl")}`.toLocaleLowerCase();
    return terms.every((term) => text.includes(term));
  });
}
