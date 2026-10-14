from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path
from uuid import uuid4

from winmacro.models import Recording
from winmacro.settings import AppSettings


def app_data_dir() -> Path:
    root = Path.home() / ".winmacro"
    root.mkdir(parents=True, exist_ok=True)
    return root


def recordings_path() -> Path:
    return app_data_dir() / "recordings.json"


def settings_path() -> Path:
    return app_data_dir() / "settings.json"


def load_settings() -> AppSettings:
    path = settings_path()
    if not path.exists():
        return AppSettings()
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return AppSettings()
    if not isinstance(data, dict):
        return AppSettings()
    return AppSettings.from_dict(data)


def load_recordings() -> list[Recording]:
    path = recordings_path()
    if not path.exists():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    if isinstance(data, list):
        items = data
    else:
        items = data.get("recordings") or []
        if not isinstance(items, list):
            return []
    result: list[Recording] = []
    for item in items:
        if not isinstance(item, dict):
            continue
        try:
            result.append(Recording.from_dict(item))
        except Exception:
            continue
    return result


def save_recordings(recordings: list[Recording]) -> None:
    payload = {"recordings": [item.to_dict() for item in recordings]}
    text = json.dumps(payload, ensure_ascii=False, indent=2)
    path = recordings_path()
    fd, tmp = tempfile.mkstemp(dir=str(path.parent), prefix=".recordings-", suffix=".tmp")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(text)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def save_settings(settings: AppSettings) -> None:
    text = json.dumps(settings.to_dict(), ensure_ascii=False, indent=2)
    path = settings_path()
    fd, tmp = tempfile.mkstemp(dir=str(path.parent), prefix=".settings-", suffix=".tmp")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(text)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def export_recording(recording: Recording, path: Path) -> None:
    path.write_text(
        json.dumps(recording.to_dict(), ensure_ascii=False, indent=2),
        encoding="utf-8",
    )


def import_recording(path: Path) -> Recording:
    data = json.loads(path.read_text(encoding="utf-8"))
    return Recording.from_dict(data)


def ensure_unique_id(recording: Recording, existing_ids: set[str]) -> Recording:
    if recording.id not in existing_ids:
        return recording
    recording.id = uuid4().hex[:12]
    while recording.id in existing_ids:
        recording.id = uuid4().hex[:12]
    return recording
