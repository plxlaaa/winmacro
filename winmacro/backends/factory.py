from __future__ import annotations

import sys

from .base import RecorderBackend
from .native import NativeBackend, find_native_library


def create_backend() -> tuple[RecorderBackend, str]:
    lib = find_native_library()
    if lib is not None:
        try:
            return NativeBackend(lib), f"Rust 原生核心 ({lib.name})"
        except Exception:
            pass
    if sys.platform == "win32":
        from .win32_backend import Win32Backend

        return Win32Backend(), "Win32 低级钩子"
    try:
        from .pynput_backend import PynputBackend

        return PynputBackend(), "pynput 兼容引擎"
    except ImportError as exc:
        raise RuntimeError("当前环境没有可用的录制引擎") from exc
