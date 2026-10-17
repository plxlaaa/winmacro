#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod hotkeys;

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::Builder as ShortcutBuilder;
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
fn set_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    hotkey: Option<String>,
) -> Result<(), String> {
    let spec = hotkey
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let normalized = match spec {
        None => None,
        Some(s) => {
            let norm = hotkeys::normalize(s)?;
            if hotkeys::RESERVED
                .iter()
                .any(|r| norm.eq_ignore_ascii_case(r))
            {
                return Err("F10 / F11 是保留热键，不能分配给脚本".into());
            }
            let dup = state
                .recordings
                .lock()
                .unwrap()
                .iter()
                .any(|r| r.id != id && r.hotkey.as_deref().is_some_and(|h| h.eq_ignore_ascii_case(&norm)));
            if dup {
                return Err("该热键已被其他脚本使用".into());
            }
            Some(norm)
        }
    };

    {
        let mut list = state.recordings.lock().unwrap();
        let rec = list
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| "找不到该录制".to_string())?;
        rec.hotkey = normalized;
        store::save_recordings(&list)?;
    }
    hotkeys::rebind_all(&app)
}

#[tauri::command]
fn screen_size() -> (i32, i32) {
    recorder_core::primary_screen_size()
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show = MenuItem::with_id(app, "tray-show", "显示主界面", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", "退出 WinMacro", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().expect("缺应用图标").clone())
        .tooltip("WinMacro — F10 录制 / F11 停止回放")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-show" => show_main(app),
            "tray-quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    // 热键全部由 hotkeys::rebind_all 动态注册（含 F10/F11 保留键）
    let shortcuts = ShortcutBuilder::new().build();

    tauri::Builder::default()
        .plugin(shortcuts)
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
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
            set_hotkey,
            screen_size,
        ])
        .setup(|app| {
            recorder_core::ensure_dpi_aware();
            setup_tray(app)?;
            hotkeys::rebind_all(app.handle())?;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let keep = window
                    .app_handle()
                    .state::<AppState>()
                    .settings
                    .lock()
                    .unwrap()
                    .close_to_tray;
                if keep {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("运行 WinMacro 失败");
}
