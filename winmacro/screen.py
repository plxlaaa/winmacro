from __future__ import annotations

import sys
from dataclasses import dataclass

from winmacro.models import InputEvent


COMMON_RESOLUTIONS: list[tuple[int, int, str]] = [
    (1280, 720, "HD 720p"),
    (1366, 768, "HD"),
    (1600, 900, "HD+"),
    (1920, 1080, "Full HD"),
    (1920, 1200, "WUXGA"),
    (2560, 1080, "UW-FHD"),
    (2560, 1440, "2K QHD"),
    (2560, 1600, "WQXGA"),
    (3440, 1440, "UW-QHD"),
    (3840, 2160, "4K UHD"),
    (3840, 2400, "WQUXGA"),
]


@dataclass(frozen=True)
class ScreenInfo:
    width: int
    height: int
    origin_x: int = 0
    origin_y: int = 0
    virtual_x: int = 0
    virtual_y: int = 0
    virtual_width: int = 0
    virtual_height: int = 0

    @property
    def size(self) -> tuple[int, int]:
        return self.width, self.height


def enable_dpi_awareness() -> None:
    if sys.platform != "win32":
        return
    try:
        import ctypes

        ctypes.windll.shcore.SetProcessDpiAwareness(2)
        return
    except Exception:
        pass
    try:
        import ctypes

        ctypes.windll.user32.SetProcessDPIAware()
    except Exception:
        pass


def detect_screen_info() -> ScreenInfo:
    if sys.platform == "win32":
        info = _win_screen_info()
        if info is not None:
            return info
    info = _qt_screen_info()
    if info is not None:
        return info
    return ScreenInfo(1920, 1080, virtual_width=1920, virtual_height=1080)


def detect_primary_resolution() -> tuple[int, int]:
    info = detect_screen_info()
    return info.width, info.height


def resolution_label(width: int, height: int) -> str:
    for w, h, name in COMMON_RESOLUTIONS:
        if w == width and h == height:
            return f"{width} × {height}  {name}"
    return f"{width} × {height}"


def scale_point(x: int, y: int, src_w: int, src_h: int, dst_w: int, dst_h: int) -> tuple[int, int]:
    sw = max(1, src_w)
    sh = max(1, src_h)
    dw = max(1, dst_w)
    dh = max(1, dst_h)
    return int(round(x * dw / sw)), int(round(y * dh / sh))


def capture_to_canvas(
    events: list[InputEvent],
    capture: ScreenInfo,
    canvas_w: int,
    canvas_h: int,
) -> list[InputEvent]:
    shifted: list[InputEvent] = []
    for event in events:
        if event.x is None or event.y is None:
            shifted.append(event)
            continue
        shifted.append(
            InputEvent(
                kind=event.kind,
                t_ms=event.t_ms,
                x=event.x - capture.origin_x,
                y=event.y - capture.origin_y,
                button=event.button,
                key=event.key,
                dx=event.dx,
                dy=event.dy,
                vk=event.vk,
                scan=event.scan,
            )
        )
    return remap_events(shifted, capture.width, capture.height, canvas_w, canvas_h)


def canvas_to_screen(events: list[InputEvent], canvas_w: int, canvas_h: int, screen: ScreenInfo) -> list[InputEvent]:
    scaled = remap_events(events, canvas_w, canvas_h, screen.width, screen.height)
    if screen.origin_x == 0 and screen.origin_y == 0:
        return scaled
    out: list[InputEvent] = []
    for event in scaled:
        if event.x is None or event.y is None:
            out.append(event)
            continue
        out.append(
            InputEvent(
                kind=event.kind,
                t_ms=event.t_ms,
                x=event.x + screen.origin_x,
                y=event.y + screen.origin_y,
                button=event.button,
                key=event.key,
                dx=event.dx,
                dy=event.dy,
                vk=event.vk,
                scan=event.scan,
            )
        )
    return out


def remap_events(
    events: list[InputEvent],
    src_w: int,
    src_h: int,
    dst_w: int,
    dst_h: int,
) -> list[InputEvent]:
    if not events:
        return []
    if src_w == dst_w and src_h == dst_h:
        return list(events)
    remapped: list[InputEvent] = []
    for event in events:
        if event.x is None or event.y is None:
            remapped.append(event)
            continue
        nx, ny = scale_point(event.x, event.y, src_w, src_h, dst_w, dst_h)
        remapped.append(
            InputEvent(
                kind=event.kind,
                t_ms=event.t_ms,
                x=nx,
                y=ny,
                button=event.button,
                key=event.key,
                dx=event.dx,
                dy=event.dy,
                vk=event.vk,
                scan=event.scan,
            )
        )
    return remapped


def _win_screen_info() -> ScreenInfo | None:
    try:
        import ctypes
        from ctypes import wintypes

        user32 = ctypes.windll.user32
        SM_XVIRTUALSCREEN = 76
        SM_YVIRTUALSCREEN = 77
        SM_CXVIRTUALSCREEN = 78
        SM_CYVIRTUALSCREEN = 79
        width = int(user32.GetSystemMetrics(0))
        height = int(user32.GetSystemMetrics(1))
        if width <= 0 or height <= 0:
            return None
        vx = int(user32.GetSystemMetrics(SM_XVIRTUALSCREEN))
        vy = int(user32.GetSystemMetrics(SM_YVIRTUALSCREEN))
        vw = int(user32.GetSystemMetrics(SM_CXVIRTUALSCREEN)) or width
        vh = int(user32.GetSystemMetrics(SM_CYVIRTUALSCREEN)) or height
        origin_x, origin_y = 0, 0
        try:
            class MONITORINFO(ctypes.Structure):
                _fields_ = [
                    ("cbSize", wintypes.DWORD),
                    ("rcMonitor", wintypes.RECT),
                    ("rcWork", wintypes.RECT),
                    ("dwFlags", wintypes.DWORD),
                ]

            MONITOR_DEFAULTTOPRIMARY = 1
            handle = user32.MonitorFromWindow(None, MONITOR_DEFAULTTOPRIMARY)
            info = MONITORINFO()
            info.cbSize = ctypes.sizeof(MONITORINFO)
            if handle and user32.GetMonitorInfoW(handle, ctypes.byref(info)):
                origin_x = int(info.rcMonitor.left)
                origin_y = int(info.rcMonitor.top)
                width = max(1, int(info.rcMonitor.right - info.rcMonitor.left))
                height = max(1, int(info.rcMonitor.bottom - info.rcMonitor.top))
        except Exception:
            pass
        return ScreenInfo(width, height, origin_x, origin_y, vx, vy, vw, vh)
    except Exception:
        return None


def _qt_screen_info() -> ScreenInfo | None:
    try:
        from PySide6.QtGui import QGuiApplication

        screen = QGuiApplication.primaryScreen()
        if screen is None:
            return None
        dpr = float(screen.devicePixelRatio())
        geo = screen.geometry()
        virt = screen.virtualGeometry()
        width = max(1, int(round(geo.width() * dpr)))
        height = max(1, int(round(geo.height() * dpr)))
        return ScreenInfo(
            width=width,
            height=height,
            origin_x=int(round(geo.x() * dpr)),
            origin_y=int(round(geo.y() * dpr)),
            virtual_x=int(round(virt.x() * dpr)),
            virtual_y=int(round(virt.y() * dpr)),
            virtual_width=max(width, int(round(virt.width() * dpr))),
            virtual_height=max(height, int(round(virt.height() * dpr))),
        )
    except Exception:
        return None
