//! Post-recording event cleanup, ported from the Python edition
//! (`sanitize_events` / `_drop_start_release`).

use crate::model::{InputEvent, KIND_MOUSE_MOVE, KIND_MOUSE_UP};
use crate::simplify::{simplify_moves, SimplifyLevel};

/// Collapse autorepeat / duplicate events and drop unpaired releases.
pub fn sanitize_events(events: &[InputEvent]) -> Vec<InputEvent> {
    let mut out: Vec<InputEvent> = Vec::with_capacity(events.len());
    let mut last_move: Option<(i32, i32, u64)> = None;
    let mut buttons_down: Vec<String> = Vec::new();
    let mut keys_down: Vec<(Option<u16>, Option<String>)> = Vec::new();

    for ev in events {
        match ev.kind.as_str() {
            "mouse_move" => {
                if let (Some(x), Some(y)) = (ev.x, ev.y) {
                    if let Some((lx, ly, lt)) = last_move {
                        if lx == x && ly == y && ev.t_ms - lt < 250 {
                            continue;
                        }
                    }
                    last_move = Some((x, y, ev.t_ms));
                }
                out.push(ev.clone());
            }
            "mouse_down" => {
                let btn = ev.button.clone().unwrap_or_else(|| "left".into());
                if buttons_down.contains(&btn) {
                    continue; // repeated down without an up in between
                }
                buttons_down.push(btn);
                out.push(ev.clone());
            }
            "mouse_up" => {
                let btn = ev.button.clone().unwrap_or_else(|| "left".into());
                if !buttons_down.contains(&btn) {
                    continue; // stray release
                }
                buttons_down.retain(|b| *b != btn);
                out.push(ev.clone());
            }
            "key_down" => {
                let kid = ev.key_id();
                if keys_down.contains(&kid) {
                    continue; // autorepeat
                }
                keys_down.push(kid);
                out.push(ev.clone());
            }
            "key_up" => {
                let kid = ev.key_id();
                if !keys_down.contains(&kid) {
                    continue;
                }
                keys_down.retain(|k| *k != kid);
                out.push(ev.clone());
            }
            _ => out.push(ev.clone()),
        }
    }
    out
}

/// Drop the leading mouse release that leaks in from arming the recorder.
pub fn drop_start_release(events: Vec<InputEvent>) -> Vec<InputEvent> {
    let mut events = events;
    if let Some(first) = events.first() {
        if first.kind == KIND_MOUSE_UP
            && first.button.as_deref().unwrap_or("left") == "left"
            && first.t_ms <= 80
        {
            events.remove(0);
        }
    }
    events
}

/// Full pipeline applied when a recording session is saved.
/// `keep_moves == false` drops every mouse_move sample (轨迹关闭：
/// 只保留点击、滚轮与键盘).
/// `simplify_level` 在保留 move 时再做 Douglas-Peucker 折线压缩。
/// `Off` 等价于跳过简化。
pub fn finalize(
    raw: Vec<InputEvent>,
    keep_moves: bool,
    simplify_level: SimplifyLevel,
) -> Vec<InputEvent> {
    let events = drop_start_release(raw);
    let events = sanitize_events(&events);
    if keep_moves {
        simplify_moves(events, simplify_level)
    } else {
        events
            .into_iter()
            .filter(|e| e.kind != KIND_MOUSE_MOVE)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::KIND_KEY_DOWN;

    fn ev(kind: &str, t: u64, x: Option<i32>, y: Option<i32>, button: Option<&str>) -> InputEvent {
        InputEvent {
            kind: kind.into(),
            t_ms: t,
            x,
            y,
            button: button.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn drops_stray_mouse_up() {
        let out = sanitize_events(&[ev("mouse_up", 5, Some(1), Some(1), Some("left"))]);
        assert!(out.is_empty());
    }

    #[test]
    fn collapses_key_autorepeat() {
        let events = vec![
            InputEvent { kind: KIND_KEY_DOWN.into(), t_ms: 0, vk: Some(0x41), key: Some("VK_41".into()), ..Default::default() },
            InputEvent { kind: KIND_KEY_DOWN.into(), t_ms: 30, vk: Some(0x41), key: Some("VK_41".into()), ..Default::default() },
            InputEvent { kind: "key_up".into(), t_ms: 90, vk: Some(0x41), key: Some("VK_41".into()), ..Default::default() },
        ];
        let out = sanitize_events(&events);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn collapses_duplicate_moves_same_position() {
        let events = vec![
            ev("mouse_move", 0, Some(10), Some(10), None),
            ev("mouse_move", 8, Some(10), Some(10), None),
            ev("mouse_move", 16, Some(12), Some(10), None),
        ];
        let out = sanitize_events(&events);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].x, Some(12));
    }

    #[test]
    fn drop_start_release_removes_leading_up() {
        let events = vec![
            ev("mouse_up", 20, Some(1), Some(1), Some("left")),
            ev("mouse_down", 100, Some(5), Some(5), Some("left")),
        ];
        let out = drop_start_release(events);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "mouse_down");
    }

    #[test]
    fn finalize_without_moves_keeps_clicks_scroll_keys() {
        let raw = vec![
            ev("mouse_up", 10, Some(1), Some(1), Some("left")),
            ev("mouse_move", 20, Some(100), Some(100), None),
            ev("mouse_move", 40, Some(200), Some(100), None),
            ev("mouse_down", 60, Some(200), Some(100), Some("left")),
            ev("mouse_scroll", 80, Some(200), Some(100), None),
            InputEvent { kind: KIND_KEY_DOWN.into(), t_ms: 120, vk: Some(0x41), ..Default::default() },
            ev("mouse_up", 200, Some(200), Some(100), Some("left")),
        ];
        let out = finalize(raw, false, SimplifyLevel::Medium);
        assert!(out.iter().all(|e| e.kind != "mouse_move"));
        assert_eq!(out.len(), 4); // down + scroll + key_down + up
    }

    #[test]
    fn finalize_with_moves_keeps_moves() {
        let raw = vec![ev("mouse_move", 20, Some(100), Some(100), None)];
        let out = finalize(raw, true, SimplifyLevel::Off);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn finalize_with_high_simplify_keeps_only_endpoints() {
        // 100 个共线 move + 起始 1 个 leading up 应被 drop_start_release 去掉
        // 然后 100 个共线点 → 只剩 2 个端点
        let mut raw: Vec<InputEvent> = (0..100)
            .map(|i| ev("mouse_move", i * 5, Some(i as i32), Some(0), None))
            .collect();
        // 在最前面塞一个会被 drop_start_release 去掉的 mouse_up
        raw.insert(
            0,
            ev("mouse_up", 5, Some(0), Some(0), Some("left")),
        );
        let out = finalize(raw, true, SimplifyLevel::High);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].x, Some(0));
        assert_eq!(out[1].x, Some(99));
    }

    #[test]
    fn finalize_off_simplify_is_passthrough() {
        let raw: Vec<InputEvent> = (0..50)
            .map(|i| ev("mouse_move", i * 2, Some(i as i32), Some(0), None))
            .collect();
        let out = finalize(raw.clone(), true, SimplifyLevel::Off);
        assert_eq!(out.len(), raw.len());
    }
}
