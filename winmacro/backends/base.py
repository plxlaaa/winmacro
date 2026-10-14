from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Callable

from winmacro.models import InputEvent


class RecorderBackend(ABC):
    @abstractmethod
    def set_ignore_rect(self, x: int, y: int, w: int, h: int) -> None:
        raise NotImplementedError

    @abstractmethod
    def start(self, on_event: Callable[[InputEvent], None] | None = None) -> None:
        raise NotImplementedError

    @abstractmethod
    def stop(self) -> list[InputEvent]:
        raise NotImplementedError

    @abstractmethod
    def is_recording(self) -> bool:
        raise NotImplementedError

    @abstractmethod
    def play(
        self,
        events: list[InputEvent],
        abort_check: Callable[[], bool] | None = None,
        speed: float = 1.0,
    ) -> None:
        raise NotImplementedError

    def abort_playback(self) -> None:
        return None

    def event_count(self) -> int:
        return 0

    def shutdown(self) -> None:
        return None
