from __future__ import annotations

from dataclasses import asdict, dataclass, field
from datetime import datetime
from typing import Any
from uuid import uuid4


EVENT_LABELS = {
    "mouse_move": "鼠标移动",
    "mouse_down": "鼠标按下",
    "mouse_up": "鼠标松开",
    "mouse_scroll": "鼠标滚轮",
    "key_down": "键盘按下",
    "key_up": "键盘松开",
}

BUTTON_LABELS = {
    "left": "左键",
    "right": "右键",
    "middle": "中键",
}


@dataclass
class InputEvent:
    kind: str
    t_ms: int
    x: int | None = None
    y: int | None = None
    button: str | None = None
    key: str | None = None
    dx: int | None = None
    dy: int | None = None
    vk: int | None = None
    scan: int | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "InputEvent":
        return cls(
            kind=str(data.get("kind", "")),
            t_ms=_opt_int(data.get("t_ms")) or 0,
            x=_opt_int(data.get("x")),
            y=_opt_int(data.get("y")),
            button=_opt_str(data.get("button")),
            key=_opt_str(data.get("key")),
            dx=_opt_int(data.get("dx")),
            dy=_opt_int(data.get("dy")),
            vk=_opt_int(data.get("vk")),
            scan=_opt_int(data.get("scan")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {k: v for k, v in asdict(self).items() if v is not None or k in ("kind", "t_ms")}

    def label(self) -> str:
        return EVENT_LABELS.get(self.kind, self.kind)

    def position_text(self) -> str:
        if self.x is None or self.y is None:
            return "—"
        return f"{self.x}, {self.y}"

    def extra_text(self) -> str:
        parts: list[str] = []
        if self.button:
            parts.append(BUTTON_LABELS.get(self.button, self.button))
        if self.key:
            parts.append(self.key)
        if self.kind == "mouse_scroll" and self.dy is not None:
            parts.append(f"dy={self.dy}")
        return "  ".join(parts) if parts else "—"

    def time_text(self) -> str:
        sec = self.t_ms / 1000.0
        return f"{sec:.3f}s"

    def key_id(self) -> str:
        if self.vk is not None:
            return f"vk:{self.vk}"
        return f"key:{self.key or ''}"


@dataclass
class Recording:
    id: str
    name: str
    created_at: str
    duration_ms: int
    events: list[InputEvent] = field(default_factory=list)
    screen_width: int = 0
    screen_height: int = 0
    resolution_mode: str = "auto"

    @classmethod
    def create(
        cls,
        events: list[InputEvent],
        name: str | None = None,
        screen_width: int = 0,
        screen_height: int = 0,
        resolution_mode: str = "auto",
    ) -> "Recording":
        now = datetime.now()
        duration = events[-1].t_ms if events else 0
        stamp = now.strftime("%H:%M:%S")
        return cls(
            id=uuid4().hex[:12],
            name=name or f"操作录制 {stamp}",
            created_at=now.isoformat(timespec="seconds"),
            duration_ms=duration,
            events=events,
            screen_width=int(screen_width or 0),
            screen_height=int(screen_height or 0),
            resolution_mode=resolution_mode or "auto",
        )

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "Recording":
        if not isinstance(data, dict):
            raise TypeError("recording payload must be an object")
        events = [InputEvent.from_dict(e) for e in data.get("events", []) if isinstance(e, dict)]
        return cls(
            id=str(data.get("id") or uuid4().hex[:12]),
            name=str(data.get("name") or "未命名录制"),
            created_at=str(data.get("created_at") or datetime.now().isoformat(timespec="seconds")),
            duration_ms=_opt_int(data.get("duration_ms")) or (events[-1].t_ms if events else 0),
            events=events,
            screen_width=_opt_int(data.get("screen_width")) or 0,
            screen_height=_opt_int(data.get("screen_height")) or 0,
            resolution_mode=str(data.get("resolution_mode") or "auto"),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "name": self.name,
            "created_at": self.created_at,
            "duration_ms": self.duration_ms,
            "screen_width": self.screen_width,
            "screen_height": self.screen_height,
            "resolution_mode": self.resolution_mode,
            "events": [e.to_dict() for e in self.events],
        }

    def resolution_text(self) -> str:
        if self.screen_width > 0 and self.screen_height > 0:
            mode = "自动" if self.resolution_mode != "manual" else "手动"
            return f"{self.screen_width}×{self.screen_height} ({mode})"
        return "未记录"

    def step_count(self) -> int:
        return len(self.events)

    def duration_text(self) -> str:
        ms = self.duration_ms
        if ms < 1000:
            return f"{ms} ms"
        return f"{ms / 1000.0:.2f} s"


def _opt_int(value: Any) -> int | None:
    if value is None or value == "":
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _opt_str(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value)
    return text if text else None


def point_in_rect(x: int, y: int, rect: tuple[int, int, int, int]) -> bool:
    rx, ry, rw, rh = rect
    return rx <= x < rx + rw and ry <= y < ry + rh


def _hits(ev: InputEvent, rects: list[tuple[int, int, int, int]]) -> bool:
    if ev.x is None or ev.y is None:
        return False
    return any(point_in_rect(ev.x, ev.y, rect) for rect in rects)


def _inflate(rect: tuple[int, int, int, int], pad: int) -> tuple[int, int, int, int]:
    x, y, w, h = rect
    return x - pad, y - pad, w + pad * 2, h + pad * 2


def strip_stop_click(
    events: list[InputEvent],
    overlay_rect: tuple[int, int, int, int] | None,
    button_rect: tuple[int, int, int, int] | None = None,
) -> list[InputEvent]:
    """Remove overlay chrome and the trailing Stop-button click."""
    if not events:
        return []

    overlay_rects: list[tuple[int, int, int, int]] = []
    if overlay_rect is not None:
        overlay_rects.append(overlay_rect)

    cleaned: list[InputEvent] = []
    for ev in events:
        if ev.kind.startswith("mouse") and overlay_rects and _hits(ev, overlay_rects):
            continue
        cleaned.append(ev)

    cleaned = _drop_start_release(cleaned)

    stop_rects: list[tuple[int, int, int, int]] = []
    if button_rect is not None:
        stop_rects.append(_inflate(button_rect, 16))
    if overlay_rect is not None:
        stop_rects.append(_inflate(overlay_rect, 6))
    cleaned = _drop_trailing_stop_gesture(cleaned, stop_rects)
    return sanitize_events(cleaned)


def _drop_start_release(events: list[InputEvent]) -> list[InputEvent]:
    """Drop the Start-button mouse_up that can leak in right after arming."""
    if not events:
        return events
    first = events[0]
    if first.kind == "mouse_up" and (first.button or "left") == "left" and first.t_ms <= 80:
        return events[1:]
    return events


def _drop_trailing_stop_gesture(
    events: list[InputEvent],
    rects: list[tuple[int, int, int, int]],
) -> list[InputEvent]:
    if not events or not rects:
        return events
    last_t = events[-1].t_ms
    keep_upto = len(events)
    for i in range(len(events) - 1, -1, -1):
        ev = events[i]
        if last_t - ev.t_ms > 500:
            break
        if ev.kind not in ("mouse_down", "mouse_up", "mouse_move"):
            break
        if _hits(ev, rects):
            keep_upto = i
            continue
        break
    trimmed = events[:keep_upto]
    if not trimmed:
        return trimmed
    last = trimmed[-1]
    if last.kind == "mouse_down" and (last.button or "left") == "left":
        return trimmed[:-1]
    return trimmed


def sanitize_events(events: list[InputEvent]) -> list[InputEvent]:
    """Drop repeats, duplicate moves, and unmatched mouse_up."""
    out: list[InputEvent] = []
    keys_down: set[str] = set()
    buttons_down: set[str] = set()
    last_move: tuple[int, int] | None = None

    for ev in events:
        if ev.kind == "mouse_move":
            if ev.x is None or ev.y is None:
                continue
            pos = (ev.x, ev.y)
            if last_move == pos:
                continue
            last_move = pos
            out.append(ev)
            continue

        last_move = None

        if ev.kind == "mouse_down":
            btn = ev.button or "left"
            buttons_down.add(btn)
            out.append(ev)
            continue

        if ev.kind == "mouse_up":
            btn = ev.button or "left"
            if btn not in buttons_down:
                continue
            buttons_down.discard(btn)
            out.append(ev)
            continue

        if ev.kind == "key_down":
            kid = ev.key_id()
            if kid in keys_down:
                continue
            keys_down.add(kid)
            out.append(ev)
            continue

        if ev.kind == "key_up":
            kid = ev.key_id()
            if kid not in keys_down:
                continue
            keys_down.discard(kid)
            out.append(ev)
            continue

        out.append(ev)
    return out
