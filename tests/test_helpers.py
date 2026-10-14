from pathlib import Path

from winmacro.backends.native import _apply_speed
from winmacro.keys import is_extended_vk
from winmacro.models import InputEvent, Recording
from winmacro.settings import AppSettings
from winmacro.storage import ensure_unique_id, import_recording
from winmacro.timing import interruptible_sleep


def _moves(*times: int) -> list[InputEvent]:
    return [InputEvent("mouse_move", t, t, t) for t in times]


def test_apply_speed_half_doubles_gaps():
    out = _apply_speed(_moves(0, 200, 500), 0.5)
    assert [e.t_ms for e in out] == [0, 400, 1000]


def test_apply_speed_double_halves_gaps():
    out = _apply_speed(_moves(0, 200, 500), 2.0)
    assert [e.t_ms for e in out] == [0, 100, 250]


def test_apply_speed_instant_flattens_time():
    out = _apply_speed(_moves(0, 200, 500), 0.0)
    assert [e.t_ms for e in out] == [0, 0, 0]


def test_apply_speed_identity_returns_same_list():
    events = _moves(10, 20)
    assert _apply_speed(events, 1.0) is events


def test_apply_speed_keeps_coordinates():
    out = _apply_speed([InputEvent("mouse_down", 100, 7, 9, button="left")], 2.0)
    assert (out[0].x, out[0].y) == (7, 9)
    assert out[0].button == "left"


def test_extended_arrow_and_nav_keys():
    assert is_extended_vk(0x25)
    assert is_extended_vk(0x27)
    assert is_extended_vk(0x2E)
    assert is_extended_vk(0xA5)
    assert not is_extended_vk(0x41)
    assert not is_extended_vk(None)


def test_interruptible_sleep_zero_returns_immediately():
    assert interruptible_sleep(0) is False


def test_input_event_from_dict_tolerates_null_and_junk():
    ev = InputEvent.from_dict({"kind": "mouse_move", "t_ms": None, "x": "12", "y": None})
    assert ev.t_ms == 0
    assert ev.x == 12
    assert ev.y is None


def test_recording_from_dict_tolerates_junk_numbers():
    rec = Recording.from_dict(
        {
            "name": "x",
            "duration_ms": "bad",
            "screen_width": None,
            "screen_height": "1080",
            "events": [{"kind": "key_down", "t_ms": "7"}, "junk"],
        }
    )
    assert rec.duration_ms == 7
    assert rec.screen_width == 0
    assert rec.screen_height == 1080
    assert len(rec.events) == 1


def test_recording_from_dict_rejects_non_object():
    try:
        Recording.from_dict(["nope"])
    except TypeError:
        return
    raise AssertionError("expected TypeError")


def test_load_recordings_ignores_unknown_dict_shape():
    import json
    from unittest.mock import patch

    import winmacro.storage as storage

    for payload in ({"unexpected": 1}, {"recordings": "nope"}, [1, 2]):
        with patch.object(storage, "recordings_path") as mocked:
            mocked.return_value = Path("/tmp/opencode/winmacro-shape.json")
            mocked.return_value.parent.mkdir(parents=True, exist_ok=True)
            mocked.return_value.write_text(json.dumps(payload), encoding="utf-8")
            assert storage.load_recordings() == []


def test_save_then_load_recordings_roundtrip():
    import json
    from unittest.mock import patch

    import winmacro.storage as storage

    target = Path("/tmp/opencode/winmacro-store-test")
    target.mkdir(parents=True, exist_ok=True)
    rec = Recording.create(
        [InputEvent("mouse_move", 5, 3, 4)],
        name="saved",
        screen_width=1920,
        screen_height=1080,
    )
    with patch.object(storage, "recordings_path", return_value=target / "recordings.json"):
        storage.save_recordings([rec])
        assert storage.load_recordings()[0].name == "saved"
        storage.save_settings(AppSettings("manual", 2560, 1440))
        assert storage.load_settings().canvas_size() == (2560, 1440)


def test_interruptible_sleep_stops_when_requested():
    calls = {"n": 0}

    def stop():
        calls["n"] += 1
        return calls["n"] > 1

    assert interruptible_sleep(2.0, stop, slice_s=0.01) is True


def test_ensure_unique_id_keeps_fresh_id():
    rec = Recording.create([InputEvent("mouse_move", 0, 1, 2)])
    out = ensure_unique_id(rec, set())
    assert out.id == rec.id


def test_ensure_unique_id_replaces_collision():
    rec = Recording.create([InputEvent("mouse_move", 0, 1, 2)], name="a")
    original = rec.id
    out = ensure_unique_id(rec, {original})
    assert out.id != original


def test_import_recording_roundtrip():
    rec = Recording.create(
        [InputEvent("key_down", 10, key="a", vk=0x41)],
        name="imported",
        screen_width=1920,
        screen_height=1080,
    )
    path = Path("/tmp/opencode/winmacro-import-test.json")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        __import__("json").dumps(rec.to_dict()),
        encoding="utf-8",
    )
    loaded = import_recording(path)
    assert loaded.name == "imported"
    assert loaded.events[0].vk == 0x41
