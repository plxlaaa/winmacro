//! Recording / playback engine glue: wraps the recorder_core C-ABI in
//! session management, progress events and window show/hide choreography.

use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use winmacro_logic::filter;
use winmacro_logic::model::{Recording, Settings};
use winmacro_logic::store;
use winmacro_logic::transform::{apply_speed, remap_events};

/// Virtual-key code of F10 — the record toggle hotkey. Its key_down/key_up
/// are excluded from recordings via `recorder_set_ignore_vk`.
pub const HOTKEY_RECORD_VK: u32 = 0x79;

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub recordings: Mutex<Vec<Recording>>,
    pub playing: AtomicBool,
    pub play_token: AtomicU64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: Mutex::new(store::load_settings()),
            recordings: Mutex::new(store::load_recordings()),
            playing: AtomicBool::new(false),
            play_token: AtomicU64::new(0),
        }
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn hide_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

pub fn start_recording(app: &AppHandle) -> Result<(), String> {
    if recorder_core::recorder_is_recording() != 0 {
        return Ok(());
    }
    recorder_core::recorder_set_ignore_vk(HOTKEY_RECORD_VK);
    let code = recorder_core::recorder_start();
    if code < 0 {
        return Err(format!("无法安装全局输入钩子（错误码 {code}）"));
    }
    hide_main(app);
    let app2 = app.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(250));
        if recorder_core::recorder_is_recording() == 0 {
            break;
        }
        let _ = app2.emit("rec:progress", recorder_core::recorder_event_count());
    });
    let _ = app.emit("rec:started", ());
    Ok(())
}

pub fn stop_recording(app: &AppHandle) -> Result<Recording, String> {
    let ptr = recorder_core::recorder_stop();
    if ptr.is_null() {
        return Err("录制核心未返回数据".to_string());
    }
    let raw = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    let events: Vec<winmacro_logic::model::InputEvent> =
        serde_json::from_str(&raw).map_err(|e| format!("解析录制数据失败：{e}"))?;

    let state = app.state::<AppState>();
    let (keep_moves, canvas_w, canvas_h, mode) = {
        let s = state.settings.lock().unwrap().clone();
        let (w, h) = if s.resolution_mode == "manual" {
            (s.screen_width.max(320), s.screen_height.max(240))
        } else {
            recorder_core::primary_screen_size()
        };
        (s.record_mouse_move, w, h, s.resolution_mode.clone())
    };

    let events = filter::finalize(events, keep_moves);
    let rec = Recording::create(events, None, canvas_w, canvas_h, &mode);

    {
        let mut list = state.recordings.lock().unwrap();
        list.insert(0, rec.clone());
        store::save_recordings(&list)?;
    }
    show_main(app);
    let _ = app.emit("rec:done", &rec);
    Ok(rec)
}

pub fn toggle_recording(app: &AppHandle) {
    if recorder_core::recorder_is_recording() != 0 {
        let _ = stop_recording(app);
    } else {
        if let Err(e) = start_recording(app) {
            let _ = app.emit("rec:error", e);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn play_recording(
    app: AppHandle,
    recording: Recording,
    speed: f64,
    repeat: u32,
    wait_seconds: u32,
) -> Result<(), String> {
    if recorder_core::recorder_is_recording() != 0 {
        return Err("正在录制，无法回放".to_string());
    }
    let state = app.state::<AppState>();
    if state.playing.load(Ordering::SeqCst) {
        stop_playback(&app);
    }
    let hide = state.settings.lock().unwrap().hide_on_play;
    let token = state.play_token.fetch_add(1, Ordering::SeqCst) + 1;
    state.playing.store(true, Ordering::SeqCst);

    if hide {
        hide_main(&app);
    }
    let _ = app.emit("play:started", ());

    thread::spawn(move || {
        let finish = |app: &AppHandle, aborted: bool| {
            app.state::<AppState>()
                .playing
                .store(false, Ordering::SeqCst);
            let _ = app.emit("play:done", aborted);
            show_main(app);
        };
        // countdown phase
        for s in (1..=wait_seconds).rev() {
            let _ = app.emit("play:countdown", s);
            if !sleep_interruptible(Duration::from_secs(1), &app, token) {
                finish(&app, true);
                return;
            }
        }
        // remap to the current screen, then apply speed
        let (cw, ch) = recorder_core::primary_screen_size();
        let remapped = remap_events(&recording.events, recording.screen_width, recording.screen_height, cw, ch);
        let speeded = apply_speed(&remapped, speed);
        let payload = match serde_json::to_string(&speeded) {
            Ok(p) => p,
            Err(e) => {
                let _ = app.emit("play:error", e.to_string());
                finish(&app, true);
                return;
            }
        };
        let cstr = match std::ffi::CString::new(payload) {
            Ok(c) => c,
            Err(_) => {
                finish(&app, true);
                return;
            }
        };
        for i in 0..repeat.max(1) {
            if !still_current(&app, token) {
                finish(&app, true);
                return;
            }
            let code = recorder_core::recorder_play(cstr.as_ptr());
            if code < 0 {
                let _ = app.emit("play:error", format!("回放失败（错误码 {code}）"));
                finish(&app, true);
                return;
            }
            let _ = app.emit("play:loop", i + 1);
        }
        finish(&app, false);
    });
    Ok(())
}

pub fn stop_playback(app: &AppHandle) {
    recorder_core::recorder_abort_playback();
    let state = app.state::<AppState>();
    state.play_token.fetch_add(1, Ordering::SeqCst);
    state.playing.store(false, Ordering::SeqCst);
}

fn still_current(app: &AppHandle, token: u64) -> bool {
    app.state::<AppState>().play_token.load(Ordering::SeqCst) == token
}

fn sleep_interruptible(dur: Duration, app: &AppHandle, token: u64) -> bool {
    let deadline = std::time::Instant::now() + dur;
    loop {
        if std::time::Instant::now() >= deadline {
            return true;
        }
        if !still_current(app, token) {
            return false;
        }
        thread::sleep(Duration::from_millis(50));
    }
}
