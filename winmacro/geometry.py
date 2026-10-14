from __future__ import annotations

import sys
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from PySide6.QtWidgets import QWidget


def widget_screen_rect(widget: "QWidget") -> tuple[int, int, int, int]:
    if sys.platform == "win32":
        try:
            return _win_window_rect(widget)
        except Exception:
            pass
    return _qt_physical_rect(widget)


def child_screen_rect(window: "QWidget", child: "QWidget") -> tuple[int, int, int, int]:
    ox, oy, _, _ = widget_screen_rect(window)
    dpr = window.devicePixelRatioF()
    local = child.mapTo(window, child.rect().topLeft())
    return (
        ox + int(local.x() * dpr),
        oy + int(local.y() * dpr),
        max(1, int(child.width() * dpr)),
        max(1, int(child.height() * dpr)),
    )


def inflate_rect(
    rect: tuple[int, int, int, int], pad: int
) -> tuple[int, int, int, int]:
    x, y, w, h = rect
    return x - pad, y - pad, w + pad * 2, h + pad * 2


def _qt_physical_rect(widget: "QWidget") -> tuple[int, int, int, int]:
    dpr = widget.devicePixelRatioF()
    pos = widget.mapToGlobal(widget.rect().topLeft())
    return (
        int(pos.x() * dpr),
        int(pos.y() * dpr),
        max(1, int(widget.width() * dpr)),
        max(1, int(widget.height() * dpr)),
    )


def _win_window_rect(widget: "QWidget") -> tuple[int, int, int, int]:
    import ctypes
    from ctypes import wintypes

    hwnd = int(widget.winId())
    rect = wintypes.RECT()
    ok = ctypes.windll.user32.GetWindowRect(hwnd, ctypes.byref(rect))
    if not ok:
        raise OSError("GetWindowRect failed")
    return rect.left, rect.top, max(1, rect.right - rect.left), max(1, rect.bottom - rect.top)
