//! Playback-time transforms: speed scaling and resolution remapping,
//! ported from the Python `_apply_speed` / `canvas_to_screen`.

use crate::model::InputEvent;

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
}
