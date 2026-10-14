from winmacro.models import InputEvent, strip_stop_click


def ev(kind, t, x=None, y=None, button=None, key=None, vk=None):
    return InputEvent(kind=kind, t_ms=t, x=x, y=y, button=button, key=key, vk=vk)


def test_empty():
    assert strip_stop_click([], (0, 0, 10, 10), (0, 0, 10, 10)) == []


def test_does_not_drop_real_work_clicks_far_from_overlay():
    overlay = (0, 0, 200, 80)
    button = (100, 40, 80, 28)
    events = [
        ev("mouse_move", 0, 500, 500),
        ev("mouse_down", 10, 500, 500, "left"),
        ev("mouse_up", 20, 500, 500, "left"),
    ]
    cleaned = strip_stop_click(events, overlay, button)
    assert len(cleaned) == 3


def test_trailing_move_into_button_is_removed():
    overlay = (100, 100, 280, 92)
    button = (250, 150, 108, 32)
    events = [
        ev("key_down", 10, key="a", vk=0x41),
        ev("key_up", 20, key="a", vk=0x41),
        ev("mouse_move", 300, 260, 155),
        ev("mouse_down", 310, 260, 160, "left"),
        ev("mouse_up", 330, 262, 161, "left"),
    ]
    cleaned = strip_stop_click(events, overlay, button)
    assert [e.kind for e in cleaned] == ["key_down", "key_up"]


def test_stop_click_slightly_outside_button_still_removed():
    overlay = (100, 100, 280, 92)
    button = (250, 150, 108, 32)
    events = [
        ev("mouse_move", 0, 400, 400),
        ev("mouse_down", 10, 400, 400, "left"),
        ev("mouse_up", 20, 400, 400, "left"),
        ev("mouse_move", 200, 248, 148),
        ev("mouse_down", 210, 248, 148, "left"),
        ev("mouse_up", 230, 248, 148, "left"),
    ]
    cleaned = strip_stop_click(events, overlay, button)
    assert [e.kind for e in cleaned] == ["mouse_move", "mouse_down", "mouse_up"]
    assert cleaned[-1].x == 400


def test_keyboard_events_on_overlay_are_kept():
    overlay = (0, 0, 200, 80)
    events = [
        ev("key_down", 10, key="a", vk=0x41),
        ev("key_up", 20, key="a", vk=0x41),
    ]
    cleaned = strip_stop_click(events, overlay, overlay)
    assert len(cleaned) == 2
