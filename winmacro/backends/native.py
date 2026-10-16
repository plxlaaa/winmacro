from __future__ import annotations

import ctypes
import json
import sys
from dataclasses import replace
from pathlib import Path
from typing import Callable

from winmacro.models import InputEvent

from .base import RecorderBackend


def _candidate_paths() -> list[Path]:
    root = Path(__file__).resolve().parents[2]
    names = []
    if sys.platform == "win32":
        names = ["recorder_core.dll"]
        folders = [
            root / "native" / "recorder_core" / "target" / "release",
            root / "native" / "recorder_core" / "target" / "debug",
            root / "bin",
        ]
    else:
        names = ["librecorder_core.so", "librecorder_core.dylib"]
        folders = [
            root / "native" / "recorder_core" / "target" / "release",
            root / "native" / "recorder_core" / "target" / "debug",
            root / "bin",
        ]
    paths: list[Path] = []
    for folder in folders:
        for name in names:
            paths.append(folder / name)
    return paths


def find_native_library() -> Path | None:
    for path in _candidate_paths():
        if path.exists():
            return path
    return None


def _apply_speed(events: list[InputEvent], speed: float) -> list[InputEvent]:
    factor = float(speed) if speed and speed > 0 else 0.0
    if factor == 1.0:
        return events
    if factor <= 0.0:
        return [replace(event, t_ms=0) for event in events]
    return [replace(event, t_ms=int(round(event.t_ms / factor))) for event in events]


class NativeBackend(RecorderBackend):
    def __init__(self, library: Path | None = None) -> None:
        path = library or find_native_library()
        if path is None:
            raise FileNotFoundError("recorder_core native library not found")
        self._lib = ctypes.CDLL(str(path))
        self._lib.recorder_set_ignore_rect.argtypes = [
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_int,
        ]
        self._lib.recorder_start.restype = ctypes.c_int
        self._lib.recorder_stop.restype = ctypes.c_char_p
        self._lib.recorder_is_recording.restype = ctypes.c_int
        self._lib.recorder_event_count.restype = ctypes.c_int
        self._lib.recorder_play.argtypes = [ctypes.c_char_p]
        self._lib.recorder_play.restype = ctypes.c_int
        self._recording = False

    def set_ignore_rect(self, x: int, y: int, w: int, h: int) -> None:
        self._lib.recorder_set_ignore_rect(int(x), int(y), int(w), int(h))

    def event_count(self) -> int:
        try:
            return int(self._lib.recorder_event_count())
        except Exception:
            return 0

    def start(self, on_event: Callable[[InputEvent], None] | None = None) -> None:
        if self.is_recording():
            return
        code = self._lib.recorder_start()
        if code < 0:
            # recorder_start returns -1 when the global input hooks could not
            # be installed (blocked by policy, UIPI, another process, ...).
            # Fail loudly instead of silently recording nothing.
            raise RuntimeError(f"原生核心无法安装全局输入钩子（错误码 {code}）")
        self._recording = True

    def stop(self) -> list[InputEvent]:
        raw = self._lib.recorder_stop()
        self._recording = False
        if not raw:
            return []
        payload = json.loads(raw.decode("utf-8"))
        return [InputEvent.from_dict(item) for item in payload]

    def is_recording(self) -> bool:
        return bool(self._lib.recorder_is_recording()) or self._recording

    def play(
        self,
        events: list[InputEvent],
        abort_check: Callable[[], bool] | None = None,
        speed: float = 1.0,
    ) -> None:
        payload = _apply_speed(events, speed)
        blob = json.dumps([e.to_dict() for e in payload]).encode("utf-8")
        code = self._lib.recorder_play(blob)
        if code == -1:
            raise RuntimeError("native playback is only available on Windows")
        if code == -4:
            raise RuntimeError("already playing")
        if code < 0:
            raise RuntimeError(f"native playback failed with code {code}")

    def abort_playback(self) -> None:
        self._lib.recorder_abort_playback()

    def shutdown(self) -> None:
        self._lib.recorder_shutdown()
