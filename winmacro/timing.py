from __future__ import annotations

import time
from typing import Callable


def interruptible_sleep(
    seconds: float,
    should_stop: Callable[[], bool] | None = None,
    slice_s: float = 0.05,
) -> bool:
    if should_stop and should_stop():
        return True
    if seconds <= 0:
        return False
    deadline = time.perf_counter() + seconds
    while True:
        remaining = deadline - time.perf_counter()
        if remaining <= 0:
            return bool(should_stop and should_stop())
        time.sleep(min(slice_s, remaining))
        if should_stop and should_stop():
            return True
