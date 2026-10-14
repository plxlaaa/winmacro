from winmacro.models import InputEvent, Recording
from winmacro.screen import ScreenInfo, canvas_to_screen, capture_to_canvas, remap_events, scale_point
from winmacro.settings import AppSettings


def ev(kind, t, x=None, y=None):
    return InputEvent(kind=kind, t_ms=t, x=x, y=y)


def test_scale_point_identity():
    assert scale_point(100, 200, 1920, 1080, 1920, 1080) == (100, 200)


def test_scale_point_1080_to_1440():
    x, y = scale_point(960, 540, 1920, 1080, 2560, 1440)
    assert (x, y) == (1280, 720)


def test_remap_keeps_keyboard_events():
    events = [ev("key_down", 10), ev("mouse_move", 20, 100, 50)]
    out = remap_events(events, 1920, 1080, 3840, 2160)
    assert out[0].kind == "key_down"
    assert out[0].x is None
    assert (out[1].x, out[1].y) == (200, 100)


def test_remap_identity_returns_copy():
    events = [ev("mouse_move", 0, 1, 2)]
    out = remap_events(events, 1920, 1080, 1920, 1080)
    assert out == events
    assert out is not events


def test_capture_to_canvas_shifts_origin_then_scales():
    capture = ScreenInfo(1920, 1080, origin_x=100, origin_y=50)
    events = [ev("mouse_move", 0, 1060, 590)]
    out = capture_to_canvas(events, capture, 3840, 2160)
    assert (out[0].x, out[0].y) == (1920, 1080)


def test_canvas_to_screen_adds_origin():
    screen = ScreenInfo(1920, 1080, origin_x=100, origin_y=40)
    events = [ev("mouse_move", 0, 960, 540)]
    out = canvas_to_screen(events, 1920, 1080, screen)
    assert (out[0].x, out[0].y) == (1060, 580)


def test_settings_auto_and_manual():
    auto = AppSettings.from_dict({"resolution_mode": "auto"})
    assert auto.is_auto()
    manual = AppSettings.from_dict(
        {"resolution_mode": "manual", "screen_width": 2560, "screen_height": 1440}
    )
    assert not manual.is_auto()
    assert manual.canvas_size() == (2560, 1440)


def test_recording_stores_resolution():
    rec = Recording.create([ev("mouse_move", 0, 1, 2)], screen_width=2560, screen_height=1440, resolution_mode="manual")
    loaded = Recording.from_dict(rec.to_dict())
    assert loaded.screen_width == 2560
    assert loaded.screen_height == 1440
    assert loaded.resolution_mode == "manual"
    assert "2560×1440" in loaded.resolution_text()
