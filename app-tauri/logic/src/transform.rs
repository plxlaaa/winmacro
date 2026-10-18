//! Playback-time transforms: speed scaling, resolution remapping, and
//! window-anchored remapping (v2.5+).
//!
//! Ported from the Python `_apply_speed` / `canvas_to_screen` /
//! `remap_with_anchor`.

use crate::model::InputEvent;

/// 一个目标窗口的当前屏幕位置（与客户区原点）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowBounds {
    /// 窗口左上角在屏幕坐标系下的 x。
    pub x: i32,
    /// 窗口左上角在屏幕坐标系下的 y。
    pub y: i32,
    /// 窗口客户区宽度（像素）。
    pub w: i32,
    /// 窗口客户区高度（像素）。
    pub h: i32,
}

impl WindowBounds {
    /// `ScreenToClient` 风格的换算：把客户区坐标转为屏幕坐标。
    /// 注：实际实现里 `anchor_lookup` 回调应直接返回 *屏幕* 坐标的 win_x/win_y 之和。
    /// 这里保留方法以备未来做裁剪/缩放适配（DPI 变化等）。
    pub fn to_screen(&self, win_x: i32, win_y: i32) -> (i32, i32) {
        (self.x + win_x, self.y + win_y)
    }
}

/// Scale inter-event gaps by `1/speed`. `speed <= 0` collapses everything
/// to t=0 ("立即执行").
pub fn apply_speed(events: &[InputEvent], speed: f64) -> Vec<InputEvent> {
    let factor = if speed.is_finite() && speed > 0.0 { speed } else { 0.0 };
    if factor == 1.0 {
        return events.to_vec();
    }
    let mut out = Vec::with_capacity(events.len());
    let mut orig_last = 0u64;
    let mut out_last = 0u64;
    for ev in events {
        let gap = ev.t_ms.saturating_sub(orig_last);
        orig_last = ev.t_ms;
        let scaled = if factor == 0.0 {
            0
        } else {
            ((gap as f64 / factor).round()) as u64
        };
        out_last += scaled;
        let mut next = ev.clone();
        next.t_ms = out_last;
        out.push(next);
    }
    out
}

/// Linear canvas→screen mapping with clamping.
pub fn scale_point(x: i32, y: i32, fw: i32, fh: i32, tw: i32, th: i32) -> (i32, i32) {
    if fw <= 0 || fh <= 0 || tw <= 0 || th <= 0 {
        return (x, y);
    }
    let nx = (x as i64 * tw as i64) / fw as i64;
    let ny = (y as i64 * th as i64) / fh as i64;
    (nx.clamp(0, tw as i64 - 1) as i32, ny.clamp(0, th as i64 - 1) as i32)
}

/// Remap mouse coordinates from the recorded canvas to the target screen.
pub fn remap_events(events: &[InputEvent], fw: i32, fh: i32, tw: i32, th: i32) -> Vec<InputEvent> {
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        let mut next = ev.clone();
        if ev.is_mouse() {
            if let (Some(x), Some(y)) = (ev.x, ev.y) {
                let (nx, ny) = scale_point(x, y, fw, fh, tw, th);
                next.x = Some(nx);
                next.y = Some(ny);
            }
        }
        out.push(next);
    }
    out
}

/// 窗口锚定回放：把每个 mouse 事件按其录到的窗口（target_hwnd）当前
/// 位置重新投影到屏幕坐标。回放逻辑：
///
/// 1. 若事件有 `target_hwnd` + `win_x/y`，且 `anchor_lookup(hwnd)` 返回 Some(bounds)：
///    屏幕坐标 = `bounds.x + win_x`, `bounds.y + win_y`
/// 2. 否则（无锚点 / 窗口找不到 / 关闭了 use_window_anchor）：
///    退回到 `remap_events` 的全屏线性缩放（`fw×fh` → `tw×th`）
///
/// `anchor_lookup` 用回调注入（默认 Win32 `EnumWindows`），让 logic crate
/// 不依赖平台 API，测试里可注入 mock。
pub fn remap_to_anchor<F>(
    events: &[InputEvent],
    fw: i32,
    fh: i32,
    tw: i32,
    th: i32,
    use_anchor: bool,
    mut anchor_lookup: F,
) -> Vec<InputEvent>
where
    F: FnMut(u32) -> Option<WindowBounds>,
{
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        let mut next = ev.clone();
        if ev.is_mouse() {
            // 尝试窗口锚定
            let anchored = if use_anchor {
                ev.target_hwnd
                    .and_then(|h| anchor_lookup(h))
                    .and_then(|bounds| match (ev.win_x, ev.win_y) {
                        (Some(wx), Some(wy)) => Some(bounds.to_screen(wx, wy)),
                        _ => None,
                    })
            } else {
                None
            };

            if let Some((nx, ny)) = anchored {
                next.x = Some(nx);
                next.y = Some(ny);
            } else if let (Some(x), Some(y)) = (ev.x, ev.y) {
                // 退路：全屏缩放
                let (nx, ny) = scale_point(x, y, fw, fh, tw, th);
                next.x = Some(nx);
                next.y = Some(ny);
            }
        }
        out.push(next);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::InputEvent;

    fn moves(times: &[u64]) -> Vec<InputEvent> {
        times
            .iter()
            .map(|&t| InputEvent {
                kind: "mouse_move".into(),
                t_ms: t,
                x: Some(t as i32),
                y: Some(t as i32),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn half_speed_doubles_gaps() {
        let out = apply_speed(&moves(&[0, 200, 500]), 0.5);
        assert_eq!(out.iter().map(|e| e.t_ms).collect::<Vec<_>>(), vec![0, 400, 1000]);
    }

    #[test]
    fn double_speed_halves_gaps() {
        let out = apply_speed(&moves(&[0, 200, 500]), 2.0);
        assert_eq!(out.iter().map(|e| e.t_ms).collect::<Vec<_>>(), vec![0, 100, 250]);
    }

    #[test]
    fn instant_flattens() {
        let out = apply_speed(&moves(&[0, 200, 500]), 0.0);
        assert_eq!(out.iter().map(|e| e.t_ms).collect::<Vec<_>>(), vec![0, 0, 0]);
    }

    #[test]
    fn identity() {
        assert_eq!(scale_point(100, 200, 1920, 1080, 1920, 1080), (100, 200));
    }

    #[test]
    fn upscale_1080_to_1440() {
        assert_eq!(scale_point(960, 540, 1920, 1080, 2560, 1440), (1280, 720));
    }

    #[test]
    fn clamps_out_of_range() {
        assert_eq!(scale_point(5000, 5000, 1920, 1080, 1280, 720), (1279, 719));
    }

    #[test]
    fn remap_keeps_keyboard_events() {
        let events = vec![
            InputEvent { kind: "key_down".into(), t_ms: 10, ..Default::default() },
            InputEvent { kind: "mouse_move".into(), t_ms: 20, x: Some(960), y: Some(540), ..Default::default() },
        ];
        let out = remap_events(&events, 1920, 1080, 3840, 2160);
        assert_eq!(out[0].x, None);
        assert_eq!((out[1].x.unwrap(), out[1].y.unwrap()), (1920, 1080));
    }

    // ── remap_to_anchor ──

    fn mouse_at(target_hwnd: Option<u32>, x: i32, y: i32, wx: Option<i32>, wy: Option<i32>) -> InputEvent {
        InputEvent {
            kind: "mouse_move".into(),
            t_ms: 0,
            x: Some(x),
            y: Some(y),
            target_hwnd,
            win_x: wx,
            win_y: wy,
            ..Default::default()
        }
    }

    #[test]
    fn anchor_remap_uses_window_position_when_known() {
        // 录的时候窗口在 (100, 200)，鼠标在客户区 (50, 30) → 屏幕 (150, 230)
        // 回放时窗口在 (500, 600) → 屏幕 (550, 630)
        let events = vec![mouse_at(Some(0xABCD), 150, 230, Some(50), Some(30))];
        let lookup = |hwnd: u32| -> Option<WindowBounds> {
            if hwnd == 0xABCD {
                Some(WindowBounds { x: 500, y: 600, w: 800, h: 600 })
            } else {
                None
            }
        };
        let out = remap_to_anchor(&events, 1920, 1080, 2560, 1440, true, lookup);
        assert_eq!(out[0].x, Some(550));
        assert_eq!(out[0].y, Some(630));
    }

    #[test]
    fn anchor_falls_back_to_screen_scaling_when_window_missing() {
        // 事件标了 hwnd 但 lookup 找不到 → 用全屏缩放
        let events = vec![mouse_at(Some(0xDEAD), 100, 200, Some(50), Some(50))];
        let lookup = |_: u32| -> Option<WindowBounds> { None };
        let out = remap_to_anchor(&events, 1920, 1080, 3840, 2160, true, lookup);
        // 1920→3840 (×2), 1080→2160 (×2)
        assert_eq!(out[0].x, Some(200));
        assert_eq!(out[0].y, Some(400));
    }

    #[test]
    fn anchor_falls_back_when_no_hwnd_recorded() {
        // 旧版录制没有 target_hwnd → 用全屏缩放
        let events = vec![mouse_at(None, 960, 540, None, None)];
        let lookup = |_: u32| -> Option<WindowBounds> { Some(WindowBounds { x: 0, y: 0, w: 100, h: 100 }) };
        let out = remap_to_anchor(&events, 1920, 1080, 1280, 720, true, lookup);
        assert_eq!(out[0].x, Some(640));
        assert_eq!(out[0].y, Some(360));
    }

    #[test]
    fn anchor_disabled_always_uses_screen_scaling() {
        // use_anchor = false → 不调 lookup，直接全屏缩放
        let events = vec![mouse_at(Some(0x1234), 150, 230, Some(50), Some(30))];
        let mut called = false;
        let lookup = |_: u32| -> Option<WindowBounds> {
            called = true;
            Some(WindowBounds { x: 9999, y: 9999, w: 1, h: 1 })
        };
        let out = remap_to_anchor(&events, 1920, 1080, 2560, 1440, false, lookup);
        assert!(!called, "anchor lookup must not be called when disabled");
        // 150*2560/1920=200, 230*1440/1080=306
        assert_eq!(out[0].x, Some(200));
        assert_eq!(out[0].y, Some(306));
    }

    #[test]
    fn anchor_partial_win_coords_fall_back() {
        // 录了 hwnd 但 win_x 缺 → 不能用锚点，回退全屏缩放
        let events = vec![mouse_at(Some(0x1234), 100, 200, None, Some(50))];
        let lookup = |_: u32| -> Option<WindowBounds> {
            Some(WindowBounds { x: 500, y: 500, w: 100, h: 100 })
        };
        let out = remap_to_anchor(&events, 1920, 1080, 3840, 2160, true, lookup);
        assert_eq!(out[0].x, Some(200));
        assert_eq!(out[0].y, Some(400));
    }

    #[test]
    fn anchor_does_not_affect_keyboard_events() {
        let events = vec![InputEvent {
            kind: "key_down".into(),
            t_ms: 0,
            target_hwnd: Some(0x1234),
            win_x: Some(50),
            win_y: Some(30),
            ..Default::default()
        }];
        let lookup = |_: u32| -> Option<WindowBounds> { Some(WindowBounds::default()) };
        let out = remap_to_anchor(&events, 1920, 1080, 3840, 2160, true, lookup);
        // 键鼠事件不动 x/y
        assert_eq!(out[0].x, None);
        assert_eq!(out[0].y, None);
        // target_hwnd 等附加字段原样保留
        assert_eq!(out[0].target_hwnd, Some(0x1234));
    }

    #[test]
    fn window_bounds_to_screen_basic() {
        let b = WindowBounds { x: 100, y: 200, w: 800, h: 600 };
        assert_eq!(b.to_screen(0, 0), (100, 200));
        assert_eq!(b.to_screen(799, 599), (899, 799));
    }
}
