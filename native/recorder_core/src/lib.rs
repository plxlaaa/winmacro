//! High-performance Windows low-level input recorder / player.
//! Exposes a C ABI so Python can load the DLL via ctypes.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, Ordering},
    Mutex,
};
use std::time::Instant;

// `#[serde(default)]` is required: the Python side (InputEvent.to_dict) omits
// fields whose value is None, so keys like "button"/"vk" are absent from the
// JSON payload. Without this attribute, serde rejects missing keys even for
// Option fields and recorder_play would fail with code -3 on every recording.
#[derive(Clone, Default, Serialize, Deserialize)]
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
    /// Win32 窗口句柄（v2.5+ 用于回放锚定）。
    #[serde(default)]
    pub target_hwnd: Option<u32>,
    /// 客户区 x。
    #[serde(default)]
    pub win_x: Option<i32>,
    /// 客户区 y。
    #[serde(default)]
    pub win_y: Option<i32>,
}

struct IgnoreRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

struct Engine {
    recording: AtomicBool,
    playing: AtomicBool,
    start: Mutex<Option<Instant>>,
    events: Mutex<Vec<InputEvent>>,
    ignore: Mutex<IgnoreRect>,
    last_move_ms: AtomicI32,
    /// Hotkeys (e.g. the record-toggle key) whose key_down/key_up must be
    /// excluded from the recording itself. 0 = disabled.
    ignore_vk: AtomicU32,
}

impl Engine {
    fn new() -> Self {
        Self {
            recording: AtomicBool::new(false),
            playing: AtomicBool::new(false),
            start: Mutex::new(None),
            events: Mutex::new(Vec::new()),
            ignore: Mutex::new(IgnoreRect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            }),
            last_move_ms: AtomicI32::new(-999999),
            ignore_vk: AtomicU32::new(0),
        }
    }

    /// 当前录制启动后的毫秒数。在 Linux 编译单元（非 win_impl）里无引用。
    #[cfg_attr(not(windows), allow(dead_code))]
    fn elapsed_ms(&self) -> u64 {
        self.start
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    /// 点 (x, y) 是否落在忽略矩形内。Linux 无引用。
    #[cfg_attr(not(windows), allow(dead_code))]
    fn in_ignore(&self, x: i32, y: i32) -> bool {
        let r = self.ignore.lock().unwrap();
        if r.w <= 0 || r.h <= 0 {
            return false;
        }
        x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h
    }

    /// 把事件追加到 events vector。Linux 无引用。
    #[cfg_attr(not(windows), allow(dead_code))]
    fn push(&self, ev: InputEvent) {
        if !self.recording.load(Ordering::SeqCst) || self.playing.load(Ordering::SeqCst) {
            return;
        }
        if ev.kind.starts_with("key") {
            if let Some(vk) = ev.vk {
                if (vk as u32) == self.ignore_vk.load(Ordering::Relaxed) {
                    return;
                }
            }
        }
        if let (Some(x), Some(y)) = (ev.x, ev.y) {
            if self.in_ignore(x, y) {
                return;
            }
        }
        if ev.kind == "mouse_move" {
            let t = ev.t_ms as i32;
            let last = self.last_move_ms.load(Ordering::Relaxed);
            if t - last < 8 {
                return;
            }
            self.last_move_ms.store(t, Ordering::Relaxed);
        }
        self.events.lock().unwrap().push(ev);
    }
}

static ENGINE: Lazy<Engine> = Lazy::new(Engine::new);
static LAST_JSON: Lazy<Mutex<CString>> =
    Lazy::new(|| Mutex::new(CString::new("[]").unwrap()));

/// Optional playback progress callback, called after each event with
/// `(index, total)`. Set via `recorder_set_progress_cb`; pass `null` to clear.
///
/// Only used by the Tauri backend for the time-axis UI. The Python edition
/// never sets it, so legacy behavior is unchanged.
type ProgressCb = unsafe extern "C" fn(index: u32, total: u32);
static PROGRESS_CB: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

#[no_mangle]
pub extern "C" fn recorder_set_progress_cb(cb: Option<ProgressCb>) {
    let ptr = cb
        .map(|f| f as *const c_void as *mut c_void)
        .unwrap_or(std::ptr::null_mut());
    PROGRESS_CB.store(ptr, Ordering::Release);
}

#[cfg_attr(not(windows), allow(dead_code))]
#[inline]
fn emit_progress(index: u32, total: u32) {
    let ptr = PROGRESS_CB.load(Ordering::Acquire);
    if !ptr.is_null() {
        let cb: ProgressCb = unsafe { std::mem::transmute(ptr) };
        unsafe { cb(index, total) };
    }
}

/// 紧急停止判定：回放期间、真人（非注入）按下 Esc。
/// 提取为纯函数以便跨平台单测。VK_ESCAPE = 0x1B。
pub fn is_emergency_stop(playing: bool, msg: u32, vk: u32, injected: bool) -> bool {
    const WM_KEYDOWN: u32 = 0x0100;
    const WM_SYSKEYDOWN: u32 = 0x0104;
    const VK_ESCAPE: u32 = 0x1B;
    playing && !injected && vk == VK_ESCAPE && (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN)
}

#[no_mangle]
pub extern "C" fn recorder_set_ignore_rect(x: c_int, y: c_int, w: c_int, h: c_int) {
    let mut r = ENGINE.ignore.lock().unwrap();
    r.x = x;
    r.y = y;
    r.w = w;
    r.h = h;
}

/// Hotkey exclusion: while recording, key_down/key_up events for this virtual
/// key code are dropped (used for the record-toggle hotkey). 0 disables.
#[no_mangle]
pub extern "C" fn recorder_set_ignore_vk(vk: u32) {
    ENGINE.ignore_vk.store(vk, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn recorder_is_recording() -> c_int {
    if ENGINE.recording.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn recorder_event_count() -> c_int {
    ENGINE.events.lock().unwrap().len() as c_int
}

#[cfg(windows)]
mod win_impl {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
        KEYEVENTF_KEYUP, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
        MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, PostThreadMessageW,
        SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG,
        MSLLHOOKSTRUCT, SYSTEM_METRICS_INDEX, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
        WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static MOUSE_HOOK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    static KEY_HOOK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    static HOOK_THREAD: Mutex<Option<u32>> = Mutex::new(None);

    /// 抓取鼠标事件发生时的"前台窗口 + 客户区坐标"用于回放锚定。
    /// 失败时（无前台窗口 / 坐标换算失败）三个字段都为 None。
    /// 注意：调用本函数的开销约 1 µs（两次 Win32 调用），可接受。
    fn capture_anchor(x: i32, y: i32) -> (Option<u32>, Option<i32>, Option<i32>) {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return (None, None, None);
            }
            let mut pt = POINT { x, y };
            // windows 0.58: ScreenToClient 返回 BOOL (i32 newtype)，
            // 非零 = 成功，零 = 失败。As 不到 Result，所以用 .0 != 0。
            if ScreenToClient(hwnd, &mut pt).0 != 0 {
                (Some(hwnd.0 as u32), Some(pt.x), Some(pt.y))
            } else {
                (Some(hwnd.0 as u32), None, None)
            }
        }
    }

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && ENGINE.recording.load(Ordering::SeqCst) {
            let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            if (info.flags & 0x00000003) != 0 {
                return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
            }
            let x = info.pt.x;
            let y = info.pt.y;
            let t = ENGINE.elapsed_ms();
            let msg = wparam.0 as u32;
            let (hwnd, wx, wy) = capture_anchor(x, y);
            let ev = match msg {
                WM_MOUSEMOVE => Some(InputEvent {
                    kind: "mouse_move".into(),
                    t_ms: t,
                    x: Some(x),
                    y: Some(y),
                    button: None,
                    key: None,
                    dx: None,
                    dy: None,
                    vk: None,
                    scan: None,
                    target_hwnd: hwnd,
                    win_x: wx,
                    win_y: wy,
                }),
                WM_LBUTTONDOWN => Some(mouse_btn("mouse_down", "left", t, x, y, hwnd, wx, wy)),
                WM_LBUTTONUP => Some(mouse_btn("mouse_up", "left", t, x, y, hwnd, wx, wy)),
                WM_RBUTTONDOWN => Some(mouse_btn("mouse_down", "right", t, x, y, hwnd, wx, wy)),
                WM_RBUTTONUP => Some(mouse_btn("mouse_up", "right", t, x, y, hwnd, wx, wy)),
                WM_MBUTTONDOWN => Some(mouse_btn("mouse_down", "middle", t, x, y, hwnd, wx, wy)),
                WM_MBUTTONUP => Some(mouse_btn("mouse_up", "middle", t, x, y, hwnd, wx, wy)),
                WM_MOUSEWHEEL => {
                    let delta = ((info.mouseData >> 16) & 0xffff) as i16 as i32;
                    Some(InputEvent {
                        kind: "mouse_scroll".into(),
                        t_ms: t,
                        x: Some(x),
                        y: Some(y),
                        button: None,
                        key: None,
                        dx: Some(0),
                        dy: Some(delta),
                        vk: None,
                        scan: None,
                        target_hwnd: hwnd,
                        win_x: wx,
                        win_y: wy,
                    })
                }
                _ => None,
            };
            if let Some(ev) = ev {
                ENGINE.push(ev);
            }
        }
        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }

    fn mouse_btn(
        kind: &str,
        button: &str,
        t: u64,
        x: i32,
        y: i32,
        hwnd: Option<u32>,
        wx: Option<i32>,
        wy: Option<i32>,
    ) -> InputEvent {
        InputEvent {
            kind: kind.into(),
            t_ms: t,
            x: Some(x),
            y: Some(y),
            button: Some(button.into()),
            key: None,
            dx: None,
            dy: None,
            vk: None,
            scan: None,
            target_hwnd: hwnd,
            win_x: wx,
            win_y: wy,
        }
    }

    unsafe extern "system" fn key_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let injected = (info.flags.0 & 0x00000012) != 0;

            // 紧急停止：回放期间真人按下 Esc → 立即断流，并吞掉该事件，
            // 防止 Esc 被「正在被操作的程序」响应。
            if is_emergency_stop(
                ENGINE.playing.load(Ordering::SeqCst),
                wparam.0 as u32,
                info.vkCode,
                injected,
            ) {
                ENGINE.playing.store(false, Ordering::SeqCst);
                return LRESULT(1);
            }

            if ENGINE.recording.load(Ordering::SeqCst) {
                if injected {
                    return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
                }
                let t = ENGINE.elapsed_ms();
                let msg = wparam.0 as u32;
                let kind = match msg {
                    WM_KEYDOWN | WM_SYSKEYDOWN => "key_down",
                    WM_KEYUP | WM_SYSKEYUP => "key_up",
                    _ => "",
                };
                if !kind.is_empty() {
                    ENGINE.push(InputEvent {
                        kind: kind.into(),
                        t_ms: t,
                        x: None,
                        y: None,
                        button: None,
                        key: Some(format!("VK_{:02X}", info.vkCode)),
                        dx: None,
                        dy: None,
                        vk: Some(info.vkCode as u16),
                        scan: Some(info.scanCode as u16),
                        target_hwnd: None,
                        win_x: None,
                        win_y: None,
                    });
                }
            }
        }
        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }

    fn hook_loop() {
        unsafe {
            let hmod = GetModuleHandleW(None).unwrap_or_default();
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), HINSTANCE(hmod.0), 0)
                .unwrap_or_default();
            let key = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), HINSTANCE(hmod.0), 0)
                .unwrap_or_default();
            if mouse.is_invalid() || key.is_invalid() {
                if !mouse.is_invalid() {
                    let _ = UnhookWindowsHookEx(mouse);
                }
                if !key.is_invalid() {
                    let _ = UnhookWindowsHookEx(key);
                }
                MOUSE_HOOK.store(std::ptr::null_mut(), Ordering::Release);
                KEY_HOOK.store(std::ptr::null_mut(), Ordering::Release);
                return;
            }
            MOUSE_HOOK.store(mouse.0, Ordering::Release);
            KEY_HOOK.store(key.0, Ordering::Release);
            *HOOK_THREAD.lock().unwrap() = Some(GetCurrentThreadId());
            let mut msg = MSG::default();
            loop {
                let ret = GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).0;
                if ret == 0 || ret == -1 {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let mouse = HHOOK(MOUSE_HOOK.load(Ordering::Acquire));
            let key = HHOOK(KEY_HOOK.load(Ordering::Acquire));
            if !mouse.is_invalid() {
                let _ = UnhookWindowsHookEx(mouse);
            }
            if !key.is_invalid() {
                let _ = UnhookWindowsHookEx(key);
            }
            MOUSE_HOOK.store(std::ptr::null_mut(), Ordering::Release);
            KEY_HOOK.store(std::ptr::null_mut(), Ordering::Release);
            *HOOK_THREAD.lock().unwrap() = None;
        }
    }

    pub fn ensure_hooks() -> bool {
        if HOOK_THREAD.lock().unwrap().is_some() {
            return true;
        }
        thread::spawn(hook_loop);
        for _ in 0..50 {
            if HOOK_THREAD.lock().unwrap().is_some() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    pub fn stop_hooks_message() {
        if let Some(tid) = *HOOK_THREAD.lock().unwrap() {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }

    fn abs_xy(x: i32, y: i32) -> (i32, i32) {
        const NORM: i32 = 65535;
        const SM_XVIRTUALSCREEN: i32 = 76;
        const SM_YVIRTUALSCREEN: i32 = 77;
        const SM_CXVIRTUALSCREEN: i32 = 78;
        const SM_CYVIRTUALSCREEN: i32 = 79;
        let left = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(SYSTEM_METRICS_INDEX(SM_XVIRTUALSCREEN)) };
        let top = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(SYSTEM_METRICS_INDEX(SM_YVIRTUALSCREEN)) };
        let sw = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(SYSTEM_METRICS_INDEX(SM_CXVIRTUALSCREEN)) };
        let sh = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(SYSTEM_METRICS_INDEX(SM_CYVIRTUALSCREEN)) };
        let ax = if sw <= 1 { x } else { (x - left) * NORM / (sw - 1) };
        let ay = if sh <= 1 { y } else { (y - top) * NORM / (sh - 1) };
        (ax, ay)
    }

    fn send_mouse(x: i32, y: i32, flags: u32, mouse_data: i32) {
        const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;
        let (ax, ay) = abs_xy(x, y);
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: ax,
                    dy: ay,
                    mouseData: mouse_data as u32,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS(
                        flags | MOUSEEVENTF_ABSOLUTE.0 | MOUSEEVENTF_MOVE.0 | MOUSEEVENTF_VIRTUALDESK,
                    ),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            let _ = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn is_extended_vk(vk: u16) -> bool {
        matches!(
            vk,
            0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26 | 0x27 | 0x28 | 0x2D | 0x2E | 0x5B | 0x5C
                | 0x5D | 0x6F | 0x90 | 0xA3 | 0xA5
        )
    }

    fn send_key(vk: u16, up: bool) {
        let mut bits: u32 = 0;
        if is_extended_vk(vk) {
            bits |= KEYEVENTF_EXTENDEDKEY.0;
        }
        if up {
            bits |= KEYEVENTF_KEYUP.0;
        }
        let flags = windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(bits);
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            let _ = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        let _ = input;
    }

    pub fn playback(events: &[InputEvent]) -> i32 {
        if events.is_empty() {
            return 0;
        }
        if ENGINE.playing.swap(true, Ordering::SeqCst) {
            return -4;
        }
        let total = events.len() as u32;
        let mut last = 0u64;
        for (i, ev) in events.iter().enumerate() {
            if !ENGINE.playing.load(Ordering::SeqCst) {
                break;
            }
            let wait = ev.t_ms.saturating_sub(last).min(5000);
            let mut slept = 0u64;
            while slept < wait {
                if !ENGINE.playing.load(Ordering::SeqCst) {
                    break;
                }
                let slice = (wait - slept).min(50);
                thread::sleep(Duration::from_millis(slice));
                slept += slice;
            }
            if !ENGINE.playing.load(Ordering::SeqCst) {
                break;
            }
            last = ev.t_ms;
            emit_progress((i + 1) as u32, total);
            match ev.kind.as_str() {
                "mouse_move" => {
                    if let (Some(x), Some(y)) = (ev.x, ev.y) {
                        send_mouse(x, y, 0, 0);
                    }
                }
                "mouse_down" => {
                    let (x, y) = (ev.x.unwrap_or(0), ev.y.unwrap_or(0));
                    let f = match ev.button.as_deref() {
                        Some("right") => MOUSEEVENTF_RIGHTDOWN.0,
                        Some("middle") => MOUSEEVENTF_MIDDLEDOWN.0,
                        _ => MOUSEEVENTF_LEFTDOWN.0,
                    };
                    send_mouse(x, y, f, 0);
                }
                "mouse_up" => {
                    let (x, y) = (ev.x.unwrap_or(0), ev.y.unwrap_or(0));
                    let f = match ev.button.as_deref() {
                        Some("right") => MOUSEEVENTF_RIGHTUP.0,
                        Some("middle") => MOUSEEVENTF_MIDDLEUP.0,
                        _ => MOUSEEVENTF_LEFTUP.0,
                    };
                    send_mouse(x, y, f, 0);
                }
                "mouse_scroll" => {
                    let (x, y) = (ev.x.unwrap_or(0), ev.y.unwrap_or(0));
                    send_mouse(x, y, MOUSEEVENTF_WHEEL.0, ev.dy.unwrap_or(0));
                }
                "key_down" => {
                    if let Some(vk) = ev.vk {
                        send_key(vk, false);
                    }
                }
                "key_up" => {
                    if let Some(vk) = ev.vk {
                        send_key(vk, true);
                    }
                }
                _ => {}
            }
        }
        ENGINE.playing.store(false, Ordering::SeqCst);
        0
    }
}

#[cfg(not(windows))]
mod win_impl {
    use super::*;
    pub fn ensure_hooks() -> bool {
        true
    }
    pub fn stop_hooks_message() {}
    pub fn playback(_events: &[InputEvent]) -> i32 {
        -1
    }
}

/// Rust-side helpers shared with the Tauri backend (rlib consumers).
/// Make the process DPI aware so hook coordinates match physical pixels.
#[cfg(windows)]
pub fn ensure_dpi_aware() -> bool {
    use windows::Win32::UI::HiDpi::{SetProcessDpiAwareness, PROCESS_SYSTEM_DPI_AWARE};
    unsafe { SetProcessDpiAwareness(PROCESS_SYSTEM_DPI_AWARE).0 != 0 }
}

#[cfg(not(windows))]
pub fn ensure_dpi_aware() -> bool {
    false
}

/// Primary screen size in physical pixels (requires a DPI-aware process).
#[cfg(windows)]
pub fn primary_screen_size() -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        (w.max(1), h.max(1))
    }
}

#[cfg(not(windows))]
pub fn primary_screen_size() -> (i32, i32) {
    (0, 0)
}

#[no_mangle]
pub extern "C" fn recorder_start() -> c_int {
    if ENGINE.recording.load(Ordering::SeqCst) {
        return 1;
    }
    if !win_impl::ensure_hooks() {
        return -1;
    }
    ENGINE.events.lock().unwrap().clear();
    ENGINE.last_move_ms.store(-999999, Ordering::Relaxed);
    *ENGINE.start.lock().unwrap() = Some(Instant::now());
    ENGINE.recording.store(true, Ordering::SeqCst);
    0
}

#[no_mangle]
pub extern "C" fn recorder_stop() -> *const c_char {
    ENGINE.recording.store(false, Ordering::SeqCst);
    let events = ENGINE.events.lock().unwrap().clone();
    let json = serde_json::to_string(&events).unwrap_or_else(|_| "[]".into());
    let cstr = CString::new(json).unwrap_or_else(|_| CString::new("[]").unwrap());
    let mut slot = LAST_JSON.lock().unwrap();
    *slot = cstr;
    slot.as_ptr()
}

#[no_mangle]
pub extern "C" fn recorder_abort_playback() {
    ENGINE.playing.store(false, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn recorder_play(json_ptr: *const c_char) -> c_int {
    if json_ptr.is_null() {
        return -2;
    }
    let json = unsafe { CStr::from_ptr(json_ptr) }.to_string_lossy();
    let events: Vec<InputEvent> = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(_) => return -3,
    };
    win_impl::playback(&events)
}

#[no_mangle]
pub extern "C" fn recorder_shutdown() {
    ENGINE.recording.store(false, Ordering::SeqCst);
    ENGINE.playing.store(false, Ordering::SeqCst);
    win_impl::stop_hooks_message();
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mirrors the exact payload shape produced by Python's InputEvent.to_dict:
    // None fields are omitted entirely, so keys are missing from the JSON.
    #[test]
    fn emergency_stop_only_for_real_human_esc_during_playback() {
        const WM_KEYDOWN: u32 = 0x0100;
        const WM_KEYUP: u32 = 0x0101;
        const VK_ESCAPE: u32 = 0x1B;
        const VK_A: u32 = 0x41;

        // 回放中真人按 Esc → 触发
        assert!(is_emergency_stop(true, WM_KEYDOWN, VK_ESCAPE, false));
        // 非回放状态（含录制中）不触发：Esc 保持原语义
        assert!(!is_emergency_stop(false, WM_KEYDOWN, VK_ESCAPE, false));
        // 脚本注入的 Esc（SendInput 带 INJECTED 标志）不触发：脚本可正常回放 Esc 键
        assert!(!is_emergency_stop(true, WM_KEYDOWN, VK_ESCAPE, true));
        // 回放中真人的其他键不触发
        assert!(!is_emergency_stop(true, WM_KEYDOWN, VK_A, false));
        // Esc 松开事件不触发（避免 key_up 二次处理）
        assert!(!is_emergency_stop(true, WM_KEYUP, VK_ESCAPE, false));
    }

    #[test]
    fn accepts_python_json_with_missing_option_fields() {
        let json = r#"[
            {"kind":"mouse_move","t_ms":5,"x":100,"y":200},
            {"kind":"mouse_down","t_ms":12,"x":100,"y":200,"button":"left"},
            {"kind":"mouse_scroll","t_ms":20,"x":1,"y":2,"dx":0,"dy":120},
            {"kind":"key_down","t_ms":30,"key":"VK_41","vk":65,"scan":30}
        ]"#;
        let events: Vec<InputEvent> =
            serde_json::from_str(json).expect("python payload must deserialize");
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].button, None);
        assert_eq!(events[0].vk, None);
        assert_eq!(events[1].button.as_deref(), Some("left"));
        assert_eq!(events[2].dy, Some(120));
        assert_eq!(events[3].vk, Some(65));
        assert_eq!(events[3].scan, Some(30));
    }

    // The reverse direction: serialized events (with explicit nulls) must stay
    // parseable by Python's InputEvent.from_dict, which tolerates null values.
    #[test]
    fn serializes_events_with_nulls_for_python() {
        let ev = InputEvent {
            kind: "mouse_move".into(),
            t_ms: 7,
            x: Some(1),
            y: Some(2),
            ..Default::default()
        };
        let text = serde_json::to_string(&ev).unwrap();
        assert!(text.contains("\"kind\":\"mouse_move\""));
        assert!(text.contains("\"t_ms\":7"));
        assert!(text.contains("\"button\":null"));
    }

    #[test]
    fn malformed_json_reports_error_not_panic() {
        let parsed: Result<Vec<InputEvent>, _> = serde_json::from_str("not json");
        assert!(parsed.is_err());
    }

    /// progress callback 接收器：用 `AtomicU32` 收集被回调的最大 index。
    /// 注意：callback 在另一个线程跑（playback 的循环），需要原子计数。
    #[test]
    fn progress_cb_can_be_set_and_cleared() {
        // 初始为 null
        recorder_set_progress_cb(None);
        let ptr0 = PROGRESS_CB.load(Ordering::Acquire);
        assert!(ptr0.is_null(), "initial progress cb must be null");

        // 设一个测试回调：每调用一次把全局计数器 +1
        static CALL_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        CALL_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
        unsafe extern "C" fn cb(_index: u32, _total: u32) {
            CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        recorder_set_progress_cb(Some(cb));
        let ptr1 = PROGRESS_CB.load(Ordering::Acquire);
        assert!(!ptr1.is_null(), "after set, progress cb must be non-null");

        // 直接通过 emit_progress 触发（playback 在非 Windows 编译单元里不可用）
        emit_progress(1, 10);
        emit_progress(2, 10);
        assert_eq!(
            CALL_COUNT.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "cb should have been called twice"
        );

        // 清掉
        recorder_set_progress_cb(None);
        let ptr2 = PROGRESS_CB.load(Ordering::Acquire);
        assert!(ptr2.is_null(), "after clear, progress cb must be null again");

        // 清掉后 emit_progress 应当是 no-op
        emit_progress(3, 10);
        assert_eq!(
            CALL_COUNT.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "cb should not have been called after clear"
        );
    }
}
