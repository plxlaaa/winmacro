//! 动态全局热键管理：录制脚本可各自绑定一个全局快捷键。
//!
//! 策略：所有热键（含保留键 F10/F11）统一在 `rebind_all` 中用
//! `on_shortcut` 动态注册并绑定各自的 handler，任何变更后全量重绑，
//! 避免局部注销的状态漂移。`Recording.hotkey` 中存储的值一律是
//! normalize 过的规范串（`Shortcut::to_string()` 输出）。

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::engine::{self, AppState};

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

/// 全量重绑：注销一切 → 逐个 on_shortcut 注册（保留键 + 脚本热键）。
pub fn rebind_all(app: &AppHandle) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    // F10：开始 / 结束录制
    let f10: Shortcut = "F10"
        .parse()
        .map_err(|e| format!("注册 F10 失败：{e}"))?;
    gs.on_shortcut(f10, |app, _, event| {
        if event.state == ShortcutState::Pressed {
            engine::toggle_recording(app);
        }
    })
        .map_err(|e| format!("注册 F10 失败：{e}"))?;

    // F11：停止回放
    let f11: Shortcut = "F11"
        .parse()
        .map_err(|e| format!("注册 F11 失败：{e}"))?;
    gs.on_shortcut(f11, |app, _, event| {
        if event.state != ShortcutState::Pressed { return; }
        engine::stop_playback(app);
        let _ = app.emit("play:stopped", ());
    })
    .map_err(|e| format!("注册 F11 失败：{e}"))?;

    // 脚本热键
    let recordings = app.state::<AppState>().recordings.lock().unwrap().clone();
    for rec in recordings {
        let Some(spec) = rec
            .hotkey
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let s: Shortcut = match spec.parse() {
            Ok(s) => s,
            Err(_) => continue, // 脏数据：跳过，不阻塞其他热键
        };
        if s.to_string().eq_ignore_ascii_case("F10")
            || s.to_string().eq_ignore_ascii_case("F11")
        {
            continue;
        }
        let rec_id = rec.id.clone();
        // 重复绑定时后注册者报错 → 跳过（数据层已防重，这是兜底）
        let _ = gs.on_shortcut(s, move |app, _, event| {
            if event.state != ShortcutState::Pressed { return; }
            let state = app.state::<AppState>();
            let rec = state
                .recordings
                .lock()
                .unwrap()
                .iter()
                .find(|r| r.id == rec_id)
                .cloned();
            if let Some(rec) = rec {
                let wait = state.settings.lock().unwrap().wait_seconds;
                if let Err(e) = engine::play_recording(app.clone(), rec, 1.0, 1, wait) {
                    let _ = app.emit("play:error", e);
                }
            }
        });
    }
    Ok(())
}
