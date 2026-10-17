import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface InputEvent {
  kind: string;
  t_ms: number;
  x?: number | null;
  y?: number | null;
  button?: string | null;
  key?: string | null;
  dx?: number | null;
  dy?: number | null;
  vk?: number | null;
  scan?: number | null;
}

export interface Recording {
  id: string;
  name: string;
  created_at: string;
  duration_ms: number;
  events: InputEvent[];
  screen_width: number;
  screen_height: number;
  resolution_mode: string;
  hotkey?: string | null;
}

export interface Settings {
  resolution_mode: string;
  screen_width: number;
  screen_height: number;
  wait_seconds: number;
  hide_on_play: boolean;
  record_mouse_move: boolean;
  auto_check_updates: boolean;
  close_to_tray: boolean;
}

export const KIND_LABELS: Record<string, string> = {
  mouse_move: "鼠标移动",
  mouse_down: "鼠标按下",
  mouse_up: "鼠标松开",
  mouse_scroll: "鼠标滚轮",
  key_down: "键盘按下",
  key_up: "键盘松开",
};

export const BUTTON_LABELS: Record<string, string> = {
  left: "左键",
  right: "右键",
  middle: "中键",
};

export function fmtDuration(ms: number): string {
  if (ms < 1000) return `${ms} 毫秒`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)} 秒`;
  const m = Math.floor(s / 60);
  return `${m} 分 ${Math.round(s - m * 60)} 秒`;
}

export const api = {
  listRecordings: () => invoke<Recording[]>("list_recordings"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (s: Settings) => invoke<void>("set_settings", { settings: s }),
  deleteRecording: (id: string) => invoke<void>("delete_recording", { id }),
  renameRecording: (id: string, name: string) =>
    invoke<void>("rename_recording", { id, name }),
  setHotkey: (id: string, hotkey: string | null) =>
    invoke<void>("set_hotkey", { id, hotkey }),
  updateRecordingEvents: (id: string, events: InputEvent[]) =>
    invoke<void>("update_recording_events", { id, events }),
  isRecording: () => invoke<boolean>("is_recording"),
  isPlaying: () => invoke<boolean>("is_playing"),
  beginRecording: () => invoke<void>("begin_recording"),
  endRecording: () => invoke<Recording>("end_recording"),
  abortPlayback: () => invoke<void>("abort_playback"),
  startPlayback: (
    id: string,
    speed: number,
    repeat: number,
    waitSeconds?: number,
  ) =>
    invoke<void>("start_playback", {
      id,
      speed,
      repeat,
      waitSeconds: waitSeconds ?? null,
    }),
  exportRecording: (id: string, path: string) =>
    invoke<void>("export_recording", { id, path }),
  importRecording: (path: string) => invoke<Recording>("import_recording", { path }),
  screenSize: () => invoke<[number, number]>("screen_size"),
};

export function onEvent(name: string, handler: (payload: unknown) => void): Promise<UnlistenFn> {
  return listen(name, (e) => handler(e.payload));
}
