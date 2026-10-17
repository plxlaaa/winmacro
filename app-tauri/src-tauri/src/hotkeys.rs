//! 动态全局热键管理：录制脚本可各自绑定一个全局快捷键。
//!
//! 策略：所有热键统一动态注册（含保留键 F10/F11），任何变更后
//! `rebind_all` 全量重绑，避免局部注销的状态漂移。存储在
//! `Recording.hotkey` 中的值一律是 normalize 过的规范串（即
//! `Shortcut::to_string()` 的输出），保证 handler 匹配两侧对齐。

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::engine::AppState;

/// 应用保留热键，不可分配给脚本。
pub const RESERVED: [&str; 2] = ["F10", "F11"];

/// 把用户输入（如 "Control+Alt+Digit1"）解析并规范化为 Shortcut 显示串。
pub fn normalize(spec: &str) -> Result<String, String> {
    let s: Shortcut = spec
        .trim()
        .parse()
        .map_err(|_| format!("无法识别的快捷键：{spec}"))?;
    Ok(s.to_string())
}

/// 全量重绑：注销一切 → 注册保留键 → 注册所有脚本热键（冲突者跳过）。
pub fn rebind_all(app: &AppHandle) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    for key in RESERVED {
        let s: Shortcut = key
            .parse()
            .map_err(|e| format!("注册保留热键 {key} 失败：{e}"))?;
        gs.register_shortcut(s)
            .map_err(|e| format!("注册保留热键 {key} 失败：{e}"))?;
    }

    let recordings = app.state::<AppState>().recordings.lock().unwrap().clone();
    for rec in &recordings {
        if let Some(spec) = rec.hotkey.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let s: Shortcut = match spec.parse() {
                Ok(s) => s,
                Err(_) => continue, // 脏数据：跳过，不阻塞其他热键
            };
            if s.to_string().eq_ignore_ascii_case("F10")
                || s.to_string().eq_ignore_ascii_case("F11")
            {
                continue;
            }
            // 重复绑定时后注册的失败 → 跳过（数据层已防重，这是兜底）
            let _ = gs.register_shortcut(s);
        }
    }
    Ok(())
}

/// 按规范热键串查找对应录制 id（大小写不敏感）。
pub fn find_recording_id(app: &AppHandle, canonical: &str) -> Option<String> {
    let list = app.state::<AppState>().recordings.lock().unwrap().clone();
    list.iter()
        .find(|r| {
            r.hotkey
                .as_deref()
                .map(|h| h.eq_ignore_ascii_case(canonical))
                .unwrap_or(false)
        })
        .map(|r| r.id.clone())
}
