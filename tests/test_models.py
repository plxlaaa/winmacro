from winmacro.models import InputEvent, Recording, sanitize_events, strip_stop_click


def _ev(kind, t, x=None, y=None, button=None, key=None, vk=None):
    return InputEvent(kind=kind, t_ms=t, x=x, y=y, button=button, key=key, vk=vk)


def test_strip_stop_click_drops_trailing_button_press():
    overlay = (100, 100, 280, 92)
    button = (250, 150, 108, 32)
    events = [
        _ev("mouse_move", 10, 10, 10),
        _ev("mouse_down", 20, 10, 10, "left"),
        _ev("mouse_up", 40, 10, 10, "left"),
        _ev("key_down", 80, key="a", vk=0x41),
        _ev("key_up", 120, key="a", vk=0x41),
        _ev("mouse_move", 200, 260, 160),
        _ev("mouse_down", 210, 260, 160, "left"),
        _ev("mouse_up", 230, 260, 160, "left"),
    ]
    cleaned = strip_stop_click(events, overlay, button)
    kinds = [e.kind for e in cleaned]
    assert kinds == ["mouse_move", "mouse_down", "mouse_up", "key_down", "key_up"]
    assert all(not (e.x == 260 and e.y == 160) for e in cleaned)


def test_strip_ignores_events_inside_overlay():
    overlay = (0, 0, 200, 80)
    events = [
        _ev("mouse_move", 0, 10, 10),
        _ev("mouse_down", 5, 20, 20, "left"),
        _ev("mouse_move", 20, 400, 400),
        _ev("key_down", 30, key="x", vk=0x58),
        _ev("key_up", 40, key="x", vk=0x58),
    ]
    cleaned = strip_stop_click(events, overlay, overlay)
    assert [e.kind for e in cleaned] == ["mouse_move", "key_down", "key_up"]
    assert cleaned[0].x == 400


def test_recording_roundtrip():
    events = [_ev("mouse_move", 0, 1, 2), _ev("key_down", 15, key="a", button=None)]
    rec = Recording.create(events, name="demo")
    data = rec.to_dict()
    loaded = Recording.from_dict(data)
    assert loaded.name == "demo"
    assert loaded.step_count() == 2
    assert loaded.events[1].key == "a"


def test_drops_start_button_mouseup_leak():
    overlay = (100, 100, 280, 92)
    button = (250, 150, 108, 32)
    events = [
        _ev("mouse_up", 12, 260, 160, "left"),
        _ev("mouse_move", 80, 500, 500),
        _ev("mouse_down", 100, 500, 500, "left"),
        _ev("mouse_up", 120, 500, 500, "left"),
    ]
    cleaned = strip_stop_click(events, overlay, button)
    assert [e.kind for e in cleaned] == ["mouse_move", "mouse_down", "mouse_up"]
    assert cleaned[0].x == 500


def test_unmatched_mouseup_is_removed():
    events = [
        _ev("mouse_up", 5, 10, 10, "left"),
        _ev("mouse_down", 20, 10, 10, "left"),
        _ev("mouse_up", 40, 10, 10, "left"),
    ]
    cleaned = sanitize_events(events)
    assert [e.kind for e in cleaned] == ["mouse_down", "mouse_up"]


def test_key_autorepeat_collapsed():
    events = [
        _ev("key_down", 0, key="a", vk=0x41),
        _ev("key_down", 30, key="a", vk=0x41),
        _ev("key_down", 60, key="a", vk=0x41),
        _ev("key_up", 90, key="a", vk=0x41),
    ]
    cleaned = sanitize_events(events)
    assert [e.kind for e in cleaned] == ["key_down", "key_up"]


def test_duplicate_mouse_move_collapsed():
    events = [
        _ev("mouse_move", 0, 10, 10),
        _ev("mouse_move", 8, 10, 10),
        _ev("mouse_move", 16, 12, 10),
    ]
    cleaned = sanitize_events(events)
    assert [(e.x, e.y) for e in cleaned] == [(10, 10), (12, 10)]
