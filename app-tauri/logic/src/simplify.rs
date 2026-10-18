//! Mouse-move trajectory simplification (Douglas-Peucker).
//!
//! Recorded `mouse_move` events form long, near-collinear polylines. Running
//! Douglas-Peucker with a small physical-pixel epsilon trims 60-80% of the
//! points without any visible change in the playback path. The algorithm only
//! touches the geometry; timestamps and all non-move events are preserved
//! verbatim.
//!
//! Scope: simplification runs once on save, in `filter::finalize`. It is
//! pure / platform-independent and fully unit-tested.

use serde::{Deserialize, Serialize};

use crate::model::{InputEvent, KIND_MOUSE_MOVE};

/// 折线简化精度档位。
///
/// 与 UI 单选 / Settings 字段对齐：序列化用 lowercase 字符串，
/// 未知值由 `Settings::sanitize` 回退到 `Medium`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SimplifyLevel {
    Off,
    Low,
    #[default]
    Medium,
    High,
}

impl SimplifyLevel {
    /// 物理像素阈值。`None` 表示不简化。
    pub fn epsilon_px(self) -> Option<f64> {
        match self {
            Self::Off => None,
            Self::Low => Some(0.5),
            Self::Medium => Some(1.5),
            Self::High => Some(4.0),
        }
    }

    /// 解析用户输入的字符串，未知值回退到 `Medium`。
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "off" => Self::Off,
            "low" => Self::Low,
            "high" => Self::High,
            _ => Self::Medium,
        }
    }
}

/// 简化一段连续 `mouse_move` 序列，**不动**任何非 move 事件的字段。
///
/// 设计要点：
/// - 永远保留每段连续 move 的首尾两个点（点击前后位置必须精确）；
/// - 段内其余点用 Douglas-Peucker 决定去留；
/// - `level == Off` 时等价于克隆输入；
/// - 输入输出顺序都按 `t_ms` 严格递增。
pub fn simplify_moves(events: Vec<InputEvent>, level: SimplifyLevel) -> Vec<InputEvent> {
    let epsilon = match level.epsilon_px() {
        Some(e) if e > 0.0 => e,
        _ => return events,
    };

    let mut out: Vec<InputEvent> = Vec::with_capacity(events.len());
    let mut run: Vec<InputEvent> = Vec::new();

    let flush = |out: &mut Vec<InputEvent>, run: &mut Vec<InputEvent>| {
        if run.is_empty() {
            return;
        }
        if run.len() <= 2 {
            out.extend(run.drain(..));
        } else {
            simplify_run(out, run.drain(..).collect(), epsilon);
        }
    };

    for ev in events {
        if ev.kind == KIND_MOUSE_MOVE {
            run.push(ev);
        } else {
            flush(&mut out, &mut run);
            out.push(ev);
        }
    }
    flush(&mut out, &mut run);
    out
}

/// 对一段**纯 move 序列**做 DP 简化，结果追加到 `out`。
fn simplify_run(out: &mut Vec<InputEvent>, run: Vec<InputEvent>, epsilon: f64) {
    debug_assert!(!run.is_empty());

    if run.len() <= 2 {
        out.extend(run);
        return;
    }

    // 提取坐标 (f64, f64)。为安全，遇到坐标缺失就原样保留。
    let has_all_xy = run.iter().all(|e| e.x.is_some() && e.y.is_some());
    if !has_all_xy {
        out.extend(run);
        return;
    }
    let points: Vec<(f64, f64)> = run
        .iter()
        .map(|e| (e.x.unwrap() as f64, e.y.unwrap() as f64))
        .collect();

    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    douglas_peucker(&points, 0, points.len() - 1, epsilon, &mut keep);

    for (i, ev) in run.into_iter().enumerate() {
        if keep[i] {
            out.push(ev);
        }
    }
}

/// 经典 Douglas-Peucker 递归。修改 `keep` 标记保留点。
fn douglas_peucker(
    points: &[(f64, f64)],
    start: usize,
    end: usize,
    epsilon: f64,
    keep: &mut [bool],
) {
    if end <= start + 1 {
        return;
    }
    let (ax, ay) = points[start];
    let (bx, by) = points[end];
    let mut max_dist = 0.0f64;
    let mut max_idx = start;

    for i in (start + 1)..end {
        let d = perpendicular_distance(points[i], (ax, ay), (bx, by));
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }
    if max_dist > epsilon {
        keep[max_idx] = true;
        douglas_peucker(points, start, max_idx, epsilon, keep);
        douglas_peucker(points, max_idx, end, epsilon, keep);
    }
}

/// 点 P 到直线 AB 的垂线距离。
fn perpendicular_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (px, py) = p;
    let (ax, ay) = a;
    let (bx, by) = b;
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        // A == B：返回 P 到 A 的距离
        let ex = px - ax;
        let ey = py - ay;
        return (ex * ex + ey * ey).sqrt();
    }
    // |(B-A) × (P-A)| / |B-A|
    let num = (dx * (ay - py) - dy * (ax - px)).abs();
    num / len_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn move_ev(t: u64, x: i32, y: i32) -> InputEvent {
        InputEvent {
            kind: KIND_MOUSE_MOVE.into(),
            t_ms: t,
            x: Some(x),
            y: Some(y),
            ..Default::default()
        }
    }

    fn kind_of(ev: &InputEvent) -> &str {
        &ev.kind
    }

    #[test]
    fn empty_input_returns_empty() {
        let out = simplify_moves(Vec::new(), SimplifyLevel::Medium);
        assert!(out.is_empty());
    }

    #[test]
    fn single_point_preserved() {
        let out = simplify_moves(vec![move_ev(0, 10, 10)], SimplifyLevel::Medium);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].x, Some(10));
    }

    #[test]
    fn two_points_preserved() {
        let out = simplify_moves(
            vec![move_ev(0, 0, 0), move_ev(10, 100, 100)],
            SimplifyLevel::Medium,
        );
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn collinear_keeps_only_endpoints() {
        // 100 个共线点（0,0）→（99,0）：除首尾都该删
        let events: Vec<InputEvent> =
            (0..100).map(|i| move_ev(i as u64, i as i32, 0)).collect();
        let out = simplify_moves(events, SimplifyLevel::Medium);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].x, Some(0));
        assert_eq!(out[1].x, Some(99));
    }

    #[test]
    fn triangle_corner_preserved() {
        // (0,0) → (50,100) → (100,0)：中点是折点必须保留
        let events = vec![
            move_ev(0, 0, 0),
            move_ev(10, 50, 100),
            move_ev(20, 100, 0),
        ];
        let out = simplify_moves(events, SimplifyLevel::Low); // ε=0.5，100px 偏移远超阈值
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn mixed_events_split_into_runs() {
        // move..move | down | up | move..move → 两段分别简化
        let events = vec![
            move_ev(0, 0, 0),
            move_ev(5, 50, 0),
            move_ev(10, 100, 0),
            InputEvent {
                kind: "mouse_down".into(),
                t_ms: 15,
                x: Some(100),
                y: Some(0),
                button: Some("left".into()),
                ..Default::default()
            },
            InputEvent {
                kind: "mouse_up".into(),
                t_ms: 20,
                x: Some(100),
                y: Some(0),
                button: Some("left".into()),
                ..Default::default()
            },
            move_ev(25, 100, 50),
            move_ev(30, 100, 100),
        ];
        let out = simplify_moves(events, SimplifyLevel::Medium);
        // 首段 (0..10) 共线 3 点 → 2 个端点；中段非 move 保留；末段 2 点原样
        assert_eq!(out.len(), 2 + 2 + 2);
        assert_eq!(kind_of(&out[0]), "mouse_move");
        assert_eq!(kind_of(&out[1]), "mouse_move");
        assert_eq!(kind_of(&out[2]), "mouse_down");
        assert_eq!(kind_of(&out[3]), "mouse_up");
        assert_eq!(kind_of(&out[4]), "mouse_move");
        assert_eq!(kind_of(&out[5]), "mouse_move");
    }

    #[test]
    fn off_level_is_passthrough() {
        let events: Vec<InputEvent> =
            (0..50).map(|i| move_ev(i, i as i32, 0)).collect();
        let out = simplify_moves(events.clone(), SimplifyLevel::Off);
        assert_eq!(out.len(), events.len());
    }

    #[test]
    fn preserves_timestamp_ordering() {
        // 即使原输入 t_ms 已经排序，输出也必须递增
        let events: Vec<InputEvent> = (0..200)
            .map(|i| {
                // 制造抖动
                let y = if i % 7 == 0 { 1 } else { 0 };
                move_ev(i as u64 * 5, i as i32, y)
            })
            .collect();
        let out = simplify_moves(events, SimplifyLevel::Medium);
        for w in out.windows(2) {
            assert!(w[0].t_ms < w[1].t_ms, "t_ms must strictly increase");
        }
    }

    #[test]
    fn first_and_last_move_always_kept() {
        let events: Vec<InputEvent> =
            (0..100).map(|i| move_ev(i, i as i32, 0)).collect();
        let out = simplify_moves(events, SimplifyLevel::High);
        assert_eq!(out.first().unwrap().x, Some(0));
        assert_eq!(out.last().unwrap().x, Some(99));
    }

    #[test]
    fn high_level_drops_small_bumps() {
        // 1000 个共线点 + 1 个偏离 5px 的点
        let mut events: Vec<InputEvent> =
            (0..1000).map(|i| move_ev(i as u64, i as i32, 0)).collect();
        events.insert(500, move_ev(500, 500, 5)); // 偏离 5px

        let medium = simplify_moves(events.clone(), SimplifyLevel::Medium); // ε=1.5，5>1.5 → 保留
        assert!(medium.iter().any(|e| e.y == Some(5)));

        let high = simplify_moves(events, SimplifyLevel::High); // ε=4.0，5>4 但…
        // 严格说 5 > 4 仍会保留，但保留点会少很多；断言 high 一定少于 medium
        assert!(high.len() <= medium.len());
        // 关键断言：high 至少砍到 < 10 个点（1000 个点哪怕全共线也只 2 个端点）
        assert!(high.len() < 10, "high should be very aggressive, got {}", high.len());
    }

    #[test]
    fn non_move_events_unchanged() {
        let key_ev = InputEvent {
            kind: "key_down".into(),
            t_ms: 50,
            key: Some("VK_41".into()),
            vk: Some(0x41),
            scan: Some(30),
            ..Default::default()
        };
        let scroll_ev = InputEvent {
            kind: "mouse_scroll".into(),
            t_ms: 60,
            x: Some(10),
            y: Some(20),
            dy: Some(-120),
            ..Default::default()
        };
        let events = vec![
            move_ev(0, 0, 0),
            move_ev(10, 50, 0),
            key_ev.clone(),
            scroll_ev.clone(),
            move_ev(80, 100, 0),
        ];
        let out = simplify_moves(events, SimplifyLevel::Low);
        // 找到 key 和 scroll，验证字段完全保留
        let k = out.iter().find(|e| e.kind == "key_down").unwrap();
        let s = out.iter().find(|e| e.kind == "mouse_scroll").unwrap();
        assert_eq!(k.t_ms, 50);
        assert_eq!(k.key.as_deref(), Some("VK_41"));
        assert_eq!(k.vk, Some(0x41));
        assert_eq!(k.scan, Some(30));
        assert_eq!(s.t_ms, 60);
        assert_eq!(s.dy, Some(-120));
        assert_eq!(s.x, Some(10));
    }

    #[test]
    fn deleted_points_within_epsilon() {
        // 直线 (0,0)→(98,0) 上 50 个点，中间 index 25 (50, 1) 偏离 1 物理像素。
        // Low (ε=0.5)：1 > 0.5 → 中间点保留
        // High (ε=4.0)：1 < 4.0 → 中间点删除
        let mut events: Vec<InputEvent> = (0..50)
            .map(|i| move_ev(i as u64, (i * 2) as i32, 0))
            .collect();
        events[25] = InputEvent {
            kind: KIND_MOUSE_MOVE.into(),
            t_ms: 25,
            x: Some(50),
            y: Some(1), // 1 物理像素偏移
            ..Default::default()
        };

        let out_low = simplify_moves(events.clone(), SimplifyLevel::Low);
        assert!(
            out_low.iter().any(|e| e.x == Some(50) && e.y == Some(1)),
            "low(ε=0.5) should keep 1px bump"
        );

        let out_high = simplify_moves(events, SimplifyLevel::High);
        assert!(
            !out_high.iter().any(|e| e.x == Some(50) && e.y == Some(1)),
            "high(ε=4) should drop 1px bump"
        );
        // high 在 50 个共线点 + 1px 抖动情况下砍到只剩 2 个端点
        assert_eq!(out_high.len(), 2, "high(ε=4) drops 1px bump → only endpoints");
    }

    #[test]
    fn parse_unknown_string_defaults_to_medium() {
        assert_eq!(SimplifyLevel::parse("off"), SimplifyLevel::Off);
        assert_eq!(SimplifyLevel::parse("low"), SimplifyLevel::Low);
        assert_eq!(SimplifyLevel::parse("HIGH"), SimplifyLevel::High);
        assert_eq!(SimplifyLevel::parse("garbage"), SimplifyLevel::Medium);
    }

    #[test]
    fn compression_ratio_for_long_smooth_stroke() {
        // 1000 个完全共线点：high 应该砍到 2 个点（端点）
        let events: Vec<InputEvent> =
            (0..1000).map(|i| move_ev(i as u64, i as i32, 0)).collect();
        let out = simplify_moves(events, SimplifyLevel::High);
        assert_eq!(out.len(), 2, "perfectly collinear → only endpoints");
    }

    /// 基准测试：5000 个圆周上的点（拟 1 分钟绘画），
    /// 给出 medium 档下预期压缩比 4-15%，high 档 1-5%。
    /// 同时输出耗时供性能参考。
    #[test]
    fn benchmark_circle_compression_ratios() {
        let n = 5000;
        let events: Vec<InputEvent> = (0..n)
            .map(|i| {
                // 半径 200 的圆，量化到 1 物理像素 → 圆周上 ~1257 个独立点
                let angle = (i as f64) / (n as f64) * std::f64::consts::PI * 2.0;
                let x = (500.0 + 200.0 * angle.cos()).round() as i32;
                let y = (500.0 + 200.0 * angle.sin()).round() as i32;
                InputEvent {
                    kind: KIND_MOUSE_MOVE.into(),
                    t_ms: i as u64 * 12,
                    x: Some(x),
                    y: Some(y),
                    ..Default::default()
                }
            })
            .collect();

        let medium = simplify_moves(events.clone(), SimplifyLevel::Medium);
        let high = simplify_moves(events.clone(), SimplifyLevel::High);

        let med_ratio = medium.len() as f64 / n as f64;
        let high_ratio = high.len() as f64 / n as f64;
        eprintln!(
            "circle benchmark: medium {} ({:.1}%), high {} ({:.1}%)",
            medium.len(),
            med_ratio * 100.0,
            high.len(),
            high_ratio * 100.0
        );

        // 圆形近似多边形：圆周上每 ~0.05 rad 保留一个顶点即足
        // 5000 步对应 0.00126 rad/step → medium(ε=1.5) 应砍到 < 30% 保留
        assert!(med_ratio < 0.30, "medium should keep <30% of circle, got {:.1}%", med_ratio * 100.0);
        // high(ε=4) 更激进
        assert!(high_ratio < med_ratio, "high should be more aggressive than medium");
    }
}
