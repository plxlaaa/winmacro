//! WinMacro domain models. JSON shapes are wire-compatible with the legacy
//! Python edition (`~/.winmacro/recordings.json` / `settings.json`), so
//! existing user data carries over untouched.

use serde::{Deserialize, Serialize};

pub const KIND_MOUSE_MOVE: &str = "mouse_move";
pub const KIND_MOUSE_DOWN: &str = "mouse_down";
pub const KIND_MOUSE_UP: &str = "mouse_up";
pub const KIND_MOUSE_SCROLL: &str = "mouse_scroll";
pub const KIND_KEY_DOWN: &str = "key_down";
pub const KIND_KEY_UP: &str = "key_up";

/// One recorded input event. Field-for-field compatible with the Python
/// `InputEvent.to_dict()` payload, where None-valued keys are omitted
/// entirely — hence `#[serde(default)]` on the struct.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InputEvent {
    pub kind: String,
    pub t_ms: u64,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub button: Option<String>,
    pub key: Option<String>,
    pub dx: Option<i32>,
    pub dy: Option<i32>,
    pub vk: Option<u16>,
    pub scan: Option<u16>,
}

impl InputEvent {
    pub fn label(&self) -> &'static str {
        match self.kind.as_str() {
            KIND_MOUSE_MOVE => "鼠标移动",
            KIND_MOUSE_DOWN => "鼠标按下",
            KIND_MOUSE_UP => "鼠标松开",
            KIND_MOUSE_SCROLL => "鼠标滚轮",
            KIND_KEY_DOWN => "键盘按下",
            _ => "键盘松开",
        }
    }

    pub fn is_mouse(&self) -> bool {
        self.kind.starts_with("mouse")
    }

    /// Stable identity used for pairing key autorepeat suppression.
    pub fn key_id(&self) -> (Option<u16>, Option<String>) {
        (self.vk, self.key.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Recording {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub duration_ms: u64,
    pub events: Vec<InputEvent>,
    pub screen_width: i32,
    pub screen_height: i32,
    pub resolution_mode: String,
    /// 用户为该录制分配的全局快捷键（如 "Control+Alt+1"）。None = 未分配。
    #[serde(default)]
    pub hotkey: Option<String>,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::from("未命名"),
            created_at: String::new(),
            duration_ms: 0,
            events: Vec::new(),
            screen_width: 0,
            screen_height: 0,
            resolution_mode: String::from("auto"),
            hotkey: None,
        }
    }
}

impl Recording {
    pub fn create(
        events: Vec<InputEvent>,
        name: Option<String>,
        screen_width: i32,
        screen_height: i32,
        resolution_mode: &str,
    ) -> Self {
        let duration = events.last().map(|e| e.t_ms).unwrap_or(0);
        let stamp = now_compact();
        Self {
            id: new_id(),
            name: name.unwrap_or_else(|| format!("操作录制 {stamp}")),
            created_at: iso_now(),
            duration_ms: duration,
            events,
            screen_width: screen_width.max(0),
            screen_height: screen_height.max(0),
            resolution_mode: if resolution_mode.is_empty() {
                "auto".into()
            } else {
                resolution_mode.into()
            },
            hotkey: None,
        }
    }

    pub fn step_count(&self) -> usize {
        self.events.len()
    }

    pub fn duration_text(&self) -> String {
        let secs = self.duration_ms as f64 / 1000.0;
        if secs < 60.0 {
            format!("{secs:.1} 秒")
        } else {
            let m = (secs / 60.0).floor() as u64;
            let s = (secs as u64) % 60;
            format!("{m} 分 {s} 秒")
        }
    }

    pub fn resolution_text(&self) -> String {
        if self.screen_width > 0 && self.screen_height > 0 {
            format!("{}×{}", self.screen_width, self.screen_height)
        } else {
            "未记录".into()
        }
    }
}

/// App settings, persisted at `~/.winmacro/settings.json`. The first three
/// fields match the legacy Python edition; the rest are new in v2.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub resolution_mode: String, // "auto" | "manual"
    pub screen_width: i32,
    pub screen_height: i32,
    /// 开始执行脚本前等待的秒数（倒计时）。
    pub wait_seconds: u32,
    /// 脚本开始执行后是否自动隐藏主窗口（结束后自动恢复）。
    pub hide_on_play: bool,
    /// 是否记录鼠标移动轨迹；关闭后仅记录点击、滚轮与键盘。
    pub record_mouse_move: bool,
    /// 启动后是否自动检查应用更新（3 秒后静默检查）。
    pub auto_check_updates: bool,
    /// 点击窗口关闭按钮时最小化到系统托盘而不是退出。
    pub close_to_tray: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            resolution_mode: "auto".into(),
            screen_width: 1920,
            screen_height: 1080,
            wait_seconds: 3,
            hide_on_play: true,
            record_mouse_move: true,
            auto_check_updates: true,
            close_to_tray: true,
        }
    }
}

impl Settings {
    pub fn is_auto(&self) -> bool {
        self.resolution_mode != "manual"
    }

    pub fn sanitize(mut self) -> Self {
        if !matches!(self.resolution_mode.as_str(), "auto" | "manual") {
            self.resolution_mode = "auto".into();
        }
        self.screen_width = self.screen_width.clamp(320, 16384);
        self.screen_height = self.screen_height.clamp(240, 16384);
        self.wait_seconds = self.wait_seconds.min(30);
        self
    }
}

pub fn new_id() -> String {
    // 12 hex chars, same shape as the Python uuid4().hex[:12] ids.
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let a = (t ^ (t >> 32)) as u32;
    let b = ((std::process::id() as u64) ^ (t & 0xffff_ffff)) as u32;
    format!("{a:08x}{b:04x}")
}

fn iso_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let (y, mo, d) = civil_from_days(days as i64);
    let (h, mi, s) = ((secs % 86400) / 3600, (secs % 3600) / 60, secs % 60);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}")
}

fn now_compact() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 按本地时区偏移量修正小时显示：读取 TZ 环境不可靠，退化为 UTC+8（主要用户时区）。
    let local = secs + 8 * 3600;
    let (h, m, s) = ((local % 86400) / 3600, (local % 3600) / 60, local % 60);
    format!("{h:02}:{m:02}:{s:02}")
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_python_event_json_with_missing_keys() {
        let json = r#"[
            {"kind":"mouse_move","t_ms":5,"x":100,"y":200},
            {"kind":"key_down","t_ms":30,"key":"VK_41","vk":65,"scan":30}
        ]"#;
        let ev: Vec<InputEvent> = serde_json::from_str(json).unwrap();
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].button, None);
        assert_eq!(ev[0].vk, None);
        assert_eq!(ev[1].vk, Some(65));
        assert_eq!(ev[1].label(), "键盘按下");
    }

    #[test]
    fn parses_legacy_recording_payload() {
        let json = r#"{
            "id":"abc123def456","name":"旧数据","created_at":"2026-10-16T12:00:00",
            "duration_ms":1500,
            "events":[{"kind":"mouse_down","t_ms":10,"x":1,"y":2,"button":"left"}],
            "screen_width":1920,"screen_height":1080,"resolution_mode":"auto"
        }"#;
        let rec: Recording = serde_json::from_str(json).unwrap();
        assert_eq!(rec.name, "旧数据");
        assert_eq!(rec.step_count(), 1);
        assert_eq!(rec.resolution_text(), "1920×1080");
    }

    #[test]
    fn auto_check_updates_setting() {
        let s = Settings::default();
        assert!(s.auto_check_updates);
        // 旧版 settings.json 没有该字段：struct 级 #[serde(default)] 取容器
        // Default impl → 老用户默认开启自动检查
        let legacy: Settings = serde_json::from_str(r#"{"resolution_mode":"auto"}"#).unwrap();
        assert!(legacy.auto_check_updates);
        // 关闭后保存/加载往返保持
        let mut s2 = Settings::default();
        s2.auto_check_updates = false;
        let back: Settings =
            serde_json::from_str(&serde_json::to_string(&s2).unwrap()).unwrap();
        assert!(!back.auto_check_updates);
    }

    #[test]
    fn hotkey_field_is_backward_compatible() {
        // 旧版 recordings.json 无 hotkey 字段：反序列化为 None
        let legacy: Vec<Recording> = serde_json::from_str(
            r#"[{"id":"a1","name":"旧","created_at":"2026-01-01","duration_ms":5,
                "events":[{"kind":"mouse_down","t_ms":1,"x":1,"y":1,"button":"left"}],
                "screen_width":1920,"screen_height":1080,"resolution_mode":"auto"}]"#,
        )
        .unwrap();
        assert_eq!(legacy[0].hotkey, None);

        // 设置热键后序列化/反序列化往返保持
        let mut rec = legacy.into_iter().next().unwrap();
        rec.hotkey = Some("Control+Alt+1".into());
        let back: Recording =
            serde_json::from_str(&serde_json::to_string(&rec).unwrap()).unwrap();
        assert_eq!(back.hotkey.as_deref(), Some("Control+Alt+1"));
    }

    #[test]
    fn settings_defaults_and_sanitize() {
        let s = Settings::default();
        assert_eq!(s.wait_seconds, 3);
        assert!(s.hide_on_play);
        assert!(s.record_mouse_move);

        let raw: Settings =
            serde_json::from_str(r#"{"resolution_mode":"weird","wait_seconds":999}"#).unwrap();
        let clean = raw.sanitize();
        assert_eq!(clean.resolution_mode, "auto");
        assert_eq!(clean.wait_seconds, 30);
    }

    #[test]
    fn serializes_events_for_recorder_core() {
        let ev = InputEvent {
            kind: "mouse_move".into(),
            t_ms: 7,
            x: Some(1),
            ..Default::default()
        };
        let text = serde_json::to_string(&ev).unwrap();
        assert!(text.contains("\"kind\":\"mouse_move\""));
        assert!(text.contains("\"t_ms\":7"));
    }
}
