from __future__ import annotations

import threading
import time
from typing import Callable

from pynput import keyboard, mouse

from winmacro.models import InputEvent, point_in_rect
from winmacro.timing import interruptible_sleep

from .base import RecorderBackend


VK_MAP = {
    "alt": 0x12,
    "alt_l": 0xA4,
    "alt_r": 0xA5,
    "alt_gr": 0xA5,
    "backspace": 0x08,
    "caps_lock": 0x14,
    "cmd": 0x5B,
    "cmd_l": 0x5B,
    "cmd_r": 0x5C,
    "ctrl": 0x11,
    "ctrl_l": 0xA2,
    "ctrl_r": 0xA3,
    "delete": 0x2E,
    "down": 0x28,
    "end": 0x23,
    "enter": 0x0D,
    "esc": 0x1B,
    "f1": 0x70,
    "f2": 0x71,
    "f3": 0x72,
    "f4": 0x73,
    "f5": 0x74,
    "f6": 0x75,
    "f7": 0x76,
    "f8": 0x77,
    "f9": 0x78,
    "f10": 0x79,
    "f11": 0x7A,
    "f12": 0x7B,
    "home": 0x24,
    "insert": 0x2D,
    "left": 0x25,
    "menu": 0x5D,
    "num_lock": 0x90,
    "page_down": 0x22,
    "page_up": 0x21,
    "pause": 0x13,
    "print_screen": 0x2C,
    "right": 0x27,
    "scroll_lock": 0x91,
    "shift": 0x10,
    "shift_l": 0xA0,
    "shift_r": 0xA1,
    "space": 0x20,
    "tab": 0x09,
    "up": 0x26,
}


class PynputBackend(RecorderBackend):
    def __init__(self) -> None:
        self._recording = False
        self._lock = threading.Lock()
        self._events: list[InputEvent] = []
        self._start = 0.0
        self._ignore = (0, 0, 0, 0)
        self._mouse_listener: mouse.Listener | None = None
        self._key_listener: keyboard.Listener | None = None
        self._on_event: Callable[[InputEvent], None] | None = None
        self._last_move_ms = -999999
        self._abort = False

    def set_ignore_rect(self, x: int, y: int, w: int, h: int) -> None:
        with self._lock:
            self._ignore = (int(x), int(y), int(w), int(h))

    def _now_ms(self) -> int:
        return int((time.perf_counter() - self._start) * 1000)

    def _ignored(self, x: int, y: int) -> bool:
        rx, ry, rw, rh = self._ignore
        if rw <= 0 or rh <= 0:
            return False
        return point_in_rect(int(x), int(y), self._ignore)

    def _emit(self, event: InputEvent) -> None:
        callback = None
        with self._lock:
            if not self._recording:
                return
            if event.x is not None and event.y is not None and self._ignored(event.x, event.y):
                return
            if event.kind == "mouse_move":
                if event.t_ms - self._last_move_ms < 8:
                    return
                self._last_move_ms = event.t_ms
            self._events.append(event)
            callback = self._on_event
        if callback:
            callback(event)

    def start(self, on_event: Callable[[InputEvent], None] | None = None) -> None:
        with self._lock:
            if self._recording:
                return
            self._on_event = on_event
            self._events = []
            self._last_move_ms = -999999
            self._start = time.perf_counter()
            self._recording = True
        self._mouse_listener = mouse.Listener(
            on_move=self._on_move,
            on_click=self._on_click,
            on_scroll=self._on_scroll,
        )
        self._key_listener = keyboard.Listener(
            on_press=self._on_press,
            on_release=self._on_release,
        )
        self._mouse_listener.start()
        self._key_listener.start()

    def stop(self) -> list[InputEvent]:
        with self._lock:
            self._recording = False
        if self._mouse_listener:
            self._mouse_listener.stop()
            self._mouse_listener = None
        if self._key_listener:
            self._key_listener.stop()
            self._key_listener = None
        with self._lock:
            return list(self._events)

    def is_recording(self) -> bool:
        return self._recording

    def event_count(self) -> int:
        with self._lock:
            return len(self._events)

    def play(
        self,
        events: list[InputEvent],
        abort_check: Callable[[], bool] | None = None,
        speed: float = 1.0,
    ) -> None:
        if not events:
            return
        mouse_ctl = mouse.Controller()
        key_ctl = keyboard.Controller()
        last = 0
        self._abort = False
        factor = float(speed) if speed and speed > 0 else 0.0

        def should_stop() -> bool:
            return self._abort or bool(abort_check and abort_check())

        for event in events:
            if should_stop():
                break
            if factor:
                wait = max(0, event.t_ms - last) / 1000.0 / factor
            else:
                wait = 0.0
            if interruptible_sleep(wait, should_stop):
                break
            last = event.t_ms
            if should_stop():
                break
            self._play_one(event, mouse_ctl, key_ctl)

    def abort_playback(self) -> None:
        self._abort = True

    def shutdown(self) -> None:
        if self._recording:
            self.stop()

    def _on_move(self, x: float, y: float) -> None:
        self._emit(
            InputEvent(kind="mouse_move", t_ms=self._now_ms(), x=int(x), y=int(y))
        )

    def _on_click(self, x: float, y: float, button: mouse.Button, pressed: bool) -> None:
        self._emit(
            InputEvent(
                kind="mouse_down" if pressed else "mouse_up",
                t_ms=self._now_ms(),
                x=int(x),
                y=int(y),
                button=str(button).replace("Button.", ""),
            )
        )

    def _on_scroll(self, x: float, y: float, dx: int, dy: int) -> None:
        self._emit(
            InputEvent(
                kind="mouse_scroll",
                t_ms=self._now_ms(),
                x=int(x),
                y=int(y),
                dx=int(dx),
                dy=int(dy),
            )
        )

    def _on_press(self, key: keyboard.Key | keyboard.KeyCode) -> None:
        name, vk = _key_info(key)
        self._emit(InputEvent(kind="key_down", t_ms=self._now_ms(), key=name, vk=vk))

    def _on_release(self, key: keyboard.Key | keyboard.KeyCode) -> None:
        name, vk = _key_info(key)
        self._emit(InputEvent(kind="key_up", t_ms=self._now_ms(), key=name, vk=vk))

    def _play_one(self, event: InputEvent, mouse_ctl: mouse.Controller, key_ctl: keyboard.Controller) -> None:
        if event.kind in ("mouse_move", "mouse_down", "mouse_up", "mouse_scroll"):
            if event.x is not None and event.y is not None:
                mouse_ctl.position = (event.x, event.y)
        if event.kind == "mouse_down":
            mouse_ctl.press(_button(event.button))
        elif event.kind == "mouse_up":
            mouse_ctl.release(_button(event.button))
        elif event.kind == "mouse_scroll":
            mouse_ctl.scroll(event.dx or 0, event.dy or 0)
        elif event.kind in ("key_down", "key_up"):
            token = _to_pynput_key(event)
            if token is None:
                return
            if event.kind == "key_down":
                key_ctl.press(token)
            else:
                key_ctl.release(token)


def _button(name: str | None) -> mouse.Button:
    if name == "right":
        return mouse.Button.right
    if name == "middle":
        return mouse.Button.middle
    return mouse.Button.left


def _key_info(key: keyboard.Key | keyboard.KeyCode) -> tuple[str, int | None]:
    if isinstance(key, keyboard.KeyCode):
        vk = key.vk
        if key.char:
            return key.char, vk
        if vk:
            return f"VK_{vk:02X}", vk
        return "unknown", vk
    name = key.name if hasattr(key, "name") else str(key)
    return name, VK_MAP.get(name)


def _to_pynput_key(event: InputEvent):
    if event.key and len(event.key) == 1:
        return event.key
    if event.key and hasattr(keyboard.Key, event.key):
        return getattr(keyboard.Key, event.key)
    if event.vk is not None:
        return keyboard.KeyCode.from_vk(event.vk)
    return None
