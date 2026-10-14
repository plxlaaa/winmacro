from __future__ import annotations

import atexit
import ctypes
import sys
import threading
import time
from ctypes import wintypes
from typing import Callable

from winmacro.keys import is_extended_vk
from winmacro.models import InputEvent, point_in_rect
from winmacro.timing import interruptible_sleep

from .base import RecorderBackend

user32 = ctypes.WinDLL("user32", use_last_error=True) if sys.platform == "win32" else None
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True) if sys.platform == "win32" else None

WH_KEYBOARD_LL = 13
WH_MOUSE_LL = 14
WM_QUIT = 0x0012
WM_KEYDOWN = 0x0100
WM_KEYUP = 0x0101
WM_SYSKEYDOWN = 0x0104
WM_SYSKEYUP = 0x0105
WM_MOUSEMOVE = 0x0200
WM_LBUTTONDOWN = 0x0201
WM_LBUTTONUP = 0x0202
WM_RBUTTONDOWN = 0x0204
WM_RBUTTONUP = 0x0205
WM_MBUTTONDOWN = 0x0207
WM_MBUTTONUP = 0x0208
WM_MOUSEWHEEL = 0x020A
INPUT_MOUSE = 0
INPUT_KEYBOARD = 1
MOUSEEVENTF_MOVE = 0x0001
MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
MOUSEEVENTF_RIGHTDOWN = 0x0008
MOUSEEVENTF_RIGHTUP = 0x0010
MOUSEEVENTF_MIDDLEDOWN = 0x0020
MOUSEEVENTF_MIDDLEUP = 0x0040
MOUSEEVENTF_WHEEL = 0x0800
MOUSEEVENTF_ABSOLUTE = 0x8000
MOUSEEVENTF_VIRTUALDESK = 0x4000
KEYEVENTF_EXTENDEDKEY = 0x0001
KEYEVENTF_KEYUP = 0x0002
SM_XVIRTUALSCREEN = 76
SM_YVIRTUALSCREEN = 77
SM_CXVIRTUALSCREEN = 78
SM_CYVIRTUALSCREEN = 79
LLMHF_INJECTED = 0x00000001
LLMHF_LOWER_IL_INJECTED = 0x00000002
LLKHF_INJECTED = 0x00000010
LLKHF_LOWER_IL_INJECTED = 0x00000002

LRESULT = ctypes.c_ssize_t
ULONG_PTR = ctypes.c_uint64 if ctypes.sizeof(ctypes.c_void_p) == 8 else ctypes.c_uint32
HOOKPROC = ctypes.WINFUNCTYPE(LRESULT, ctypes.c_int, wintypes.WPARAM, wintypes.LPARAM) if sys.platform == "win32" else None


class POINT(ctypes.Structure):
    _fields_ = [("x", wintypes.LONG), ("y", wintypes.LONG)]


class MSLLHOOKSTRUCT(ctypes.Structure):
    _fields_ = [
        ("pt", POINT),
        ("mouseData", wintypes.DWORD),
        ("flags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class KBDLLHOOKSTRUCT(ctypes.Structure):
    _fields_ = [
        ("vkCode", wintypes.DWORD),
        ("scanCode", wintypes.DWORD),
        ("flags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", wintypes.LONG),
        ("dy", wintypes.LONG),
        ("mouseData", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    ]


class HARDWAREINPUT(ctypes.Structure):
    _fields_ = [
        ("uMsg", wintypes.DWORD),
        ("wParamL", wintypes.WORD),
        ("wParamH", wintypes.WORD),
    ]


class INPUTUNION(ctypes.Union):
    _fields_ = [("mi", MOUSEINPUT), ("ki", KEYBDINPUT), ("hi", HARDWAREINPUT)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", wintypes.DWORD), ("union", INPUTUNION)]


class MSG(ctypes.Structure):
    _fields_ = [
        ("hwnd", wintypes.HWND),
        ("message", wintypes.UINT),
        ("wParam", wintypes.WPARAM),
        ("lParam", wintypes.LPARAM),
        ("time", wintypes.DWORD),
        ("pt", POINT),
        ("lPrivate", wintypes.DWORD),
    ]


if sys.platform == "win32" and user32 is not None and kernel32 is not None:
    user32.SetWindowsHookExW.argtypes = [ctypes.c_int, HOOKPROC, wintypes.HINSTANCE, wintypes.DWORD]
    user32.SetWindowsHookExW.restype = wintypes.HHOOK
    user32.CallNextHookEx.argtypes = [wintypes.HHOOK, ctypes.c_int, wintypes.WPARAM, wintypes.LPARAM]
    user32.CallNextHookEx.restype = LRESULT
    user32.UnhookWindowsHookEx.argtypes = [wintypes.HHOOK]
    user32.UnhookWindowsHookEx.restype = wintypes.BOOL
    user32.GetMessageW.argtypes = [ctypes.POINTER(MSG), wintypes.HWND, wintypes.UINT, wintypes.UINT]
    user32.GetMessageW.restype = ctypes.c_int
    user32.TranslateMessage.argtypes = [ctypes.POINTER(MSG)]
    user32.DispatchMessageW.argtypes = [ctypes.POINTER(MSG)]
    user32.PostThreadMessageW.argtypes = [wintypes.DWORD, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    user32.SendInput.argtypes = [wintypes.UINT, ctypes.POINTER(INPUT), ctypes.c_int]
    user32.SendInput.restype = wintypes.UINT
    user32.GetSystemMetrics.argtypes = [ctypes.c_int]
    user32.GetSystemMetrics.restype = ctypes.c_int
    kernel32.GetModuleHandleW.argtypes = [wintypes.LPCWSTR]
    kernel32.GetModuleHandleW.restype = wintypes.HMODULE
    kernel32.GetCurrentThreadId.restype = wintypes.DWORD


class Win32Backend(RecorderBackend):
    def __init__(self) -> None:
        if sys.platform != "win32":
            raise RuntimeError("Win32Backend is only available on Windows")
        self._recording = False
        self._playing = False
        self._lock = threading.Lock()
        self._events: list[InputEvent] = []
        self._start = 0.0
        self._ignore = (0, 0, 0, 0)
        self._on_event: Callable[[InputEvent], None] | None = None
        self._last_move_ms = -999999
        self._mouse_hook = None
        self._key_hook = None
        self._thread: threading.Thread | None = None
        self._thread_id = 0
        self._ready = threading.Event()
        self._mouse_proc = HOOKPROC(self._mouse_cb)
        self._key_proc = HOOKPROC(self._key_cb)
        atexit.register(self.shutdown)

    def set_ignore_rect(self, x: int, y: int, w: int, h: int) -> None:
        with self._lock:
            self._ignore = (int(x), int(y), int(w), int(h))

    def start(self, on_event: Callable[[InputEvent], None] | None = None) -> None:
        with self._lock:
            if self._recording:
                return
            if self._playing:
                raise RuntimeError("正在回放，无法开始录制")
            self._on_event = on_event
            self._events = []
            self._last_move_ms = -999999
            self._start = time.perf_counter()
        self._ensure_hooks()
        with self._lock:
            self._recording = True

    def stop(self) -> list[InputEvent]:
        with self._lock:
            self._recording = False
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
        with self._lock:
            self._playing = True
        last = 0
        factor = float(speed) if speed and speed > 0 else 0.0

        def should_stop() -> bool:
            with self._lock:
                playing = self._playing
            return (not playing) or bool(abort_check and abort_check())

        try:
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
                self._play_one(event)
        finally:
            with self._lock:
                self._playing = False

    def abort_playback(self) -> None:
        with self._lock:
            self._playing = False

    def shutdown(self) -> None:
        self._recording = False
        self._playing = False
        tid = self._thread_id
        thread = self._thread
        if tid:
            user32.PostThreadMessageW(tid, WM_QUIT, 0, 0)
        if thread and thread.is_alive() and thread is not threading.current_thread():
            thread.join(timeout=1.0)

    def _now_ms(self) -> int:
        return int((time.perf_counter() - self._start) * 1000)

    def _ignored(self, x: int, y: int) -> bool:
        rx, ry, rw, rh = self._ignore
        if rw <= 0 or rh <= 0:
            return False
        return point_in_rect(int(x), int(y), (rx, ry, rw, rh))

    def _emit(self, event: InputEvent) -> None:
        callback = None
        with self._lock:
            if not self._recording or self._playing:
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

    def _ensure_hooks(self) -> None:
        if self._thread and self._thread.is_alive():
            return
        self._ready.clear()
        self._thread = threading.Thread(target=self._hook_loop, name="winmacro-hooks", daemon=True)
        self._thread.start()
        if not self._ready.wait(timeout=2.0) or not self._mouse_hook or not self._key_hook:
            raise RuntimeError("无法安装 Windows 低级输入钩子")

    def _hook_loop(self) -> None:
        hmod = kernel32.GetModuleHandleW(None)
        self._mouse_hook = user32.SetWindowsHookExW(WH_MOUSE_LL, self._mouse_proc, hmod, 0)
        self._key_hook = user32.SetWindowsHookExW(WH_KEYBOARD_LL, self._key_proc, hmod, 0)
        self._thread_id = kernel32.GetCurrentThreadId()
        if not self._mouse_hook or not self._key_hook:
            self._ready.set()
            return
        self._ready.set()
        msg = MSG()
        while True:
            ret = user32.GetMessageW(ctypes.byref(msg), None, 0, 0)
            if ret == 0 or ret == -1:
                break
            user32.TranslateMessage(ctypes.byref(msg))
            user32.DispatchMessageW(ctypes.byref(msg))
        if self._mouse_hook:
            user32.UnhookWindowsHookEx(self._mouse_hook)
            self._mouse_hook = None
        if self._key_hook:
            user32.UnhookWindowsHookEx(self._key_hook)
            self._key_hook = None
        self._thread_id = 0

    def _mouse_cb(self, code: int, wparam: int, lparam: int) -> int:
        try:
            return self._mouse_cb_impl(code, wparam, lparam)
        except Exception:
            return user32.CallNextHookEx(self._mouse_hook, code, wparam, lparam)

    def _mouse_cb_impl(self, code: int, wparam: int, lparam: int) -> int:
        if code >= 0 and self._recording:
            info = ctypes.cast(lparam, ctypes.POINTER(MSLLHOOKSTRUCT)).contents
            if info.flags & (LLMHF_INJECTED | LLMHF_LOWER_IL_INJECTED):
                return user32.CallNextHookEx(self._mouse_hook, code, wparam, lparam)
            x, y, t = int(info.pt.x), int(info.pt.y), self._now_ms()
            msg = int(wparam)
            if msg == WM_MOUSEMOVE:
                self._emit(InputEvent("mouse_move", t, x, y))
            elif msg == WM_LBUTTONDOWN:
                self._emit(InputEvent("mouse_down", t, x, y, button="left"))
            elif msg == WM_LBUTTONUP:
                self._emit(InputEvent("mouse_up", t, x, y, button="left"))
            elif msg == WM_RBUTTONDOWN:
                self._emit(InputEvent("mouse_down", t, x, y, button="right"))
            elif msg == WM_RBUTTONUP:
                self._emit(InputEvent("mouse_up", t, x, y, button="right"))
            elif msg == WM_MBUTTONDOWN:
                self._emit(InputEvent("mouse_down", t, x, y, button="middle"))
            elif msg == WM_MBUTTONUP:
                self._emit(InputEvent("mouse_up", t, x, y, button="middle"))
            elif msg == WM_MOUSEWHEEL:
                delta = ctypes.c_short((info.mouseData >> 16) & 0xFFFF).value
                self._emit(InputEvent("mouse_scroll", t, x, y, dy=int(delta)))
        return user32.CallNextHookEx(self._mouse_hook, code, wparam, lparam)

    def _key_cb(self, code: int, wparam: int, lparam: int) -> int:
        try:
            return self._key_cb_impl(code, wparam, lparam)
        except Exception:
            return user32.CallNextHookEx(self._key_hook, code, wparam, lparam)

    def _key_cb_impl(self, code: int, wparam: int, lparam: int) -> int:
        if code >= 0 and self._recording:
            info = ctypes.cast(lparam, ctypes.POINTER(KBDLLHOOKSTRUCT)).contents
            if info.flags & (LLKHF_INJECTED | LLKHF_LOWER_IL_INJECTED):
                return user32.CallNextHookEx(self._key_hook, code, wparam, lparam)
            t = self._now_ms()
            msg = int(wparam)
            kind = None
            if msg in (WM_KEYDOWN, WM_SYSKEYDOWN):
                kind = "key_down"
            elif msg in (WM_KEYUP, WM_SYSKEYUP):
                kind = "key_up"
            if kind:
                vk = int(info.vkCode)
                self._emit(
                    InputEvent(kind, t, key=f"VK_{vk:02X}", vk=vk, scan=int(info.scanCode))
                )
        return user32.CallNextHookEx(self._key_hook, code, wparam, lparam)

    def _abs(self, x: int, y: int) -> tuple[int, int]:
        left = user32.GetSystemMetrics(SM_XVIRTUALSCREEN)
        top = user32.GetSystemMetrics(SM_YVIRTUALSCREEN)
        width = max(1, user32.GetSystemMetrics(SM_CXVIRTUALSCREEN) - 1)
        height = max(1, user32.GetSystemMetrics(SM_CYVIRTUALSCREEN) - 1)
        return int((x - left) * 65535 / width), int((y - top) * 65535 / height)

    def _send_mouse(self, x: int, y: int, flags: int, data: int = 0) -> None:
        ax, ay = self._abs(x, y)
        inp = INPUT()
        inp.type = INPUT_MOUSE
        inp.union.mi = MOUSEINPUT(
            ax,
            ay,
            data & 0xFFFFFFFF,
            flags | MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            0,
            0,
        )
        user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))

    def _send_key(self, vk: int, up: bool) -> None:
        flags = 0
        if is_extended_vk(vk):
            flags |= KEYEVENTF_EXTENDEDKEY
        if up:
            flags |= KEYEVENTF_KEYUP
        inp = INPUT()
        inp.type = INPUT_KEYBOARD
        inp.union.ki = KEYBDINPUT(vk, 0, flags, 0, 0)
        user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))

    def _play_one(self, event: InputEvent) -> None:
        x, y = event.x or 0, event.y or 0
        if event.kind == "mouse_move":
            self._send_mouse(x, y, 0)
        elif event.kind == "mouse_down":
            flag = {
                "right": MOUSEEVENTF_RIGHTDOWN,
                "middle": MOUSEEVENTF_MIDDLEDOWN,
            }.get(event.button or "left", MOUSEEVENTF_LEFTDOWN)
            self._send_mouse(x, y, flag)
        elif event.kind == "mouse_up":
            flag = {
                "right": MOUSEEVENTF_RIGHTUP,
                "middle": MOUSEEVENTF_MIDDLEUP,
            }.get(event.button or "left", MOUSEEVENTF_LEFTUP)
            self._send_mouse(x, y, flag)
        elif event.kind == "mouse_scroll":
            self._send_mouse(x, y, MOUSEEVENTF_WHEEL, event.dy or 0)
        elif event.kind == "key_down" and event.vk is not None:
            self._send_key(event.vk, False)
        elif event.kind == "key_up" and event.vk is not None:
            self._send_key(event.vk, True)
