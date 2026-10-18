//! Recording / playback engine glue: wraps the recorder_core C-ABI in
//! session management, progress events and window show/hide choreography.

use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};
use winmacro_logic::filter;
use winmacro_logic::model::{Recording, Settings};
use winmacro_logic::store;
use winmacro_logic::transform::{apply_speed, remap_events, remap_to_anchor, WindowBounds};

/// Virtual-key code of F10 — the record toggle hotkey. Its key_down/key_up
/// are excluded from recordings via `recorder_set_ignore_vk`.
pub const HOTKEY_RECORD_VK: u32 = 0x79;

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub recordings: Mutex<Vec<Recording>>,
    pub playing: AtomicBool,
    pub play_token: AtomicU64,
    /// Last play progress emit (epoch millis) — used by the progress callback
    /// to throttle `play:progress` events.
    pub last_progress_ms: AtomicI64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: Mutex::new(store::load_settings()),
            recordings: Mutex::new(store::load_recordings()),
            playing: AtomicBool::new(false),
            play_token: AtomicU64::new(0),
            last_progress_ms: AtomicI64::new(0),
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
    let (keep_moves, canvas_w, canvas_h, mode, simplify_level) = {
        let s = state.settings.lock().unwrap().clone();
        let (w, h) = if s.resolution_mode == "manual" {
            (s.screen_width.max(320), s.screen_height.max(240))
        } else {
            recorder_core::primary_screen_size()
        };
        (
            s.record_mouse_move,
            w,
            h,
            s.resolution_mode.clone(),
            s.trajectory_simplify,
        )
    };

    let events = filter::finalize(events, keep_moves, simplify_level);
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

    // 装上 progress 回调（幂等）。每次播放前装一次即可。
    install_progress_callback(&app);

    thread::spawn(move || {
        let finish = |app: &AppHandle, aborted: bool| {
            recorder_core::recorder_set_progress_cb(None);
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
        // remap to the current screen (or to the original window's current position),
        // then apply speed
        let (cw, ch) = recorder_core::primary_screen_size();
        let use_anchor = state.settings.lock().unwrap().use_window_anchor;
        let remapped = if use_anchor {
            remap_to_anchor(
                &recording.events,
                recording.screen_width,
                recording.screen_height,
                cw,
                ch,
                true,
                |hwnd| lookup_window_bounds(hwnd),
            )
        } else {
            remap_events(
                &recording.events,
                recording.screen_width,
                recording.screen_height,
                cw,
                ch,
            )
        };
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
    // 清掉 progress 回调，避免播放停止后还有残留
    recorder_core::recorder_set_progress_cb(None);
    let state = app.state::<AppState>();
    state.play_token.fetch_add(1, Ordering::SeqCst);
    state.playing.store(false, Ordering::SeqCst);
}

/// progress 回调用的全局 AppHandle 槽位。第一次播放前由
/// `install_progress_callback` 设置。`OnceLock` 保证只设置一次。
static PROGRESS_APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

/// Tauri 后端安装 progress 回调：C ABI 函数被 recorder_core 在每个
/// 事件发完后调用；这里节流后 emit `play:progress` 事件给前端。
/// 该回调在 recorder_core 播放线程里执行，**不能**阻塞，且必须 `unsafe`。
///
/// 节流策略：每 ≥ 60ms 才发一次事件，避免 5000 个事件压垮前端。
unsafe extern "C" fn progress_trampoline(index: u32, total: u32) {
    let app = match PROGRESS_APP.get() {
        Some(a) => a,
        None => return, // 还没初始化（理论上 main 里先 install 再 play）
    };
    let state = app.state::<AppState>();

    // 节流：上次 emit 距今 < 60ms 就跳过；最后一次（index == total）必发
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let last = state.last_progress_ms.load(Ordering::Relaxed);
    if now_ms - last < 60 && index != total {
        return;
    }
    state.last_progress_ms.store(now_ms, Ordering::Relaxed);

    let _ = app.emit(
        "play:progress",
        serde_json::json!({ "index": index, "total": total }),
    );
}

/// 一次性把 progress callback 装到 recorder_core。该函数是幂等的：
/// 第一次调用设置 AppHandle + 回调；之后只刷新时间戳节流器。
pub fn install_progress_callback(app: &AppHandle) {
    let _ = PROGRESS_APP.set(app.clone());
    recorder_core::recorder_set_progress_cb(Some(progress_trampoline));
}

fn still_current(app: &AppHandle, token: u64) -> bool {
    app.state::<AppState>().play_token.load(Ordering::SeqCst) == token
}

/// 把 Win32 HWND 解析成当前屏幕位置 + 客户区大小。
/// Windows：调 `GetWindowRect`。窗口不存在 / 已销毁 → 返回 None。
/// 非 Windows：永远 None（logic crate 自动回退到全屏缩放）。
#[cfg(windows)]
fn lookup_window_bounds(hwnd: u32) -> Option<WindowBounds> {
    use recorder_core::WinOk;
    use windows::Win32::Foundation::{HWND, RECT};
    unsafe {
        let h = HWND(hwnd as *mut std::ffi::c_void);
        let mut rect = RECT::default();
        // 0.58 返回 BOOL、0.59+ 返回 Result，用 .win_ok() 兼容两者
        if windows::Win32::UI::WindowsAndMessaging::GetWindowRect(h, &mut rect).win_ok() {
            Some(WindowBounds {
                x: rect.left,
                y: rect.top,
                w: rect.right - rect.left,
                h: rect.bottom - rect.top,
            })
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
fn lookup_window_bounds(_hwnd: u32) -> Option<WindowBounds> {
    None
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
