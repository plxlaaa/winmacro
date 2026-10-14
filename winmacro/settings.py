from __future__ import annotations

from dataclasses import asdict, dataclass

from winmacro.screen import detect_primary_resolution


@dataclass
class AppSettings:
    resolution_mode: str = "auto"
    screen_width: int = 1920
    screen_height: int = 1080

    def is_auto(self) -> bool:
        return self.resolution_mode != "manual"

    def canvas_size(self) -> tuple[int, int]:
        if self.is_auto():
            return detect_primary_resolution()
        return max(320, int(self.screen_width)), max(240, int(self.screen_height))

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, data: dict | None) -> "AppSettings":
        data = data or {}
        mode = str(data.get("resolution_mode") or "auto")
        if mode not in ("auto", "manual"):
            mode = "auto"
        width = _clamp_int(data.get("screen_width"), 1920, 320, 16384)
        height = _clamp_int(data.get("screen_height"), 1080, 240, 16384)
        return cls(resolution_mode=mode, screen_width=width, screen_height=height)


def _clamp_int(value, default: int, lo: int, hi: int) -> int:
    try:
        number = int(value)
    except (TypeError, ValueError):
        return default
    return max(lo, min(hi, number))
