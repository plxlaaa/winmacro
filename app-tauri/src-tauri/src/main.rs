#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, ShortcutState};
use winmacro_logic::model::{Recording, Settings};
use winmacro_logic::store;

use engine::{play_recording, start_recording, stop_playback, stop_recording, AppState};

#[tauri::command]
fn list_recordings(state: State<'_, AppState>) -> Vec<Recording> {
    state.recordings.lock().unwrap().clone()
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    let clean = settings.sanitize();
    store::save_settings(&clean)?;
    *state.settings.lock().unwrap() = clean;
    Ok(())
}

#[tauri::command]
fn delete_recording(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut list = state.recordings.lock().unwrap();
    list.retain(|r| r.id != id);
    store::save_recordings(&list)
}

#[tauri::command]
fn rename_recording(state: State<'_, AppState>, id: String, name: String) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let mut list = state.recordings.lock().unwrap();
    if let Some(r) = list.iter_mut().find(|r| r.id == id) {
        r.name = name;
    }
    store::save_recordings(&list)
}

#[tauri::command]
fn is_recording() -> bool {
    recorder_core::recorder_is_recording() != 0
}

#[tauri::command]
fn is_playing(state: State<'_, AppState>) -> bool {
    state.playing.load(Ordering::SeqCst)
}

#[tauri::command]
fn begin_recording(app: AppHandle) -> Result<(), String> {
    start_recording(&app)
}

#[tauri::command]
fn end_recording(app: AppHandle) -> Result<Recording, String> {
    stop_recording(&app)
}

#[tauri::command]
fn abort_playback(app: AppHandle) {
    stop_playback(&app);
}

#[tauri::command]
fn start_playback(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    speed: f64,
    repeat: u32,
    wait_seconds: Option<u32>,
) -> Result<(), String> {
    let rec = state
        .recordings
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.id == id)
        .cloned()
        .ok_or_else(|| "找不到该录制".to_string())?;
    let wait = wait_seconds.unwrap_or_else(|| state.settings.lock().unwrap().wait_seconds);
    play_recording(app, rec, speed, repeat.max(1), wait)
}

#[tauri::command]
fn export_recording(state: State<'_, AppState>, id: String, path: String) -> Result<(), String> {
    let rec = state
        .recordings
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.id == id)
        .cloned()
        .ok_or_else(|| "找不到该录制".to_string())?;
    store::export_recording(&rec, &std::path::PathBuf::from(&path))
}

#[tauri::command]
fn import_recording(state: State<'_, AppState>, path: String) -> Result<Recording, String> {
    let rec = store::import_recording(&std::path::PathBuf::from(&path))?;
    let mut list = state.recordings.lock().unwrap();
    let rec = store::ensure_unique_id(rec, &list);
    list.insert(0, rec.clone());
    store::save_recordings(&list)?;
    Ok(rec)
}

#[tauri::command]
fn screen_size() -> (i32, i32) {
    recorder_core::primary_screen_size()
}

fn main() {
    let shortcuts = ShortcutBuilder::new()
        .with_shortcuts(["F10", "F11"])
        .expect("注册全局快捷键失败")
        .with_handler(|app: &AppHandle, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let name = shortcut.to_string();
            if name.contains("F10") {
                engine::toggle_recording(app);
            } else if name.contains("F11") {
                stop_playback(app);
                let _ = app.emit("play:stopped", ());
            }
        })
        .build();

    tauri::Builder::default()
        .plugin(shortcuts)
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_recordings,
            get_settings,
            set_settings,
            delete_recording,
            rename_recording,
            is_recording,
            is_playing,
            begin_recording,
            end_recording,
            abort_playback,
            start_playback,
            export_recording,
            import_recording,
            screen_size,
        ])
        .setup(|app| {
            recorder_core::ensure_dpi_aware();
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 WinMacro 失败");
}
