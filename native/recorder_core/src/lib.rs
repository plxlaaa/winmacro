//! High-performance Windows low-level input recorder / player.
//! Exposes a C ABI so Python can load the DLL via ctypes.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
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
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.start
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    fn in_ignore(&self, x: i32, y: i32) -> bool {
        let r = self.ignore.lock().unwrap();
        if r.w <= 0 || r.h <= 0 {
            return false;
        }
        x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h
    }

    fn push(&self, ev: InputEvent) {
        if !self.recording.load(Ordering::SeqCst) || self.playing.load(Ordering::SeqCst) {
            return;
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

#[no_mangle]
pub extern "C" fn recorder_set_ignore_rect(x: c_int, y: c_int, w: c_int, h: c_int) {
    let mut r = ENGINE.ignore.lock().unwrap();
    r.x = x;
    r.y = y;
    r.w = w;
    r.h = h;
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
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::{GetCurrentThreadId, PostThreadMessageW};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
        KEYEVENTF_KEYUP, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
        MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL,
        WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
        WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP,
        WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static mut MOUSE_HOOK: HHOOK = HHOOK(0);
    static mut KEY_HOOK: HHOOK = HHOOK(0);
    static HOOK_THREAD: Mutex<Option<u32>> = Mutex::new(None);

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && ENGINE.recording.load(Ordering::SeqCst) {
            let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            if (info.flags & 0x00000003) != 0 {
                return CallNextHookEx(HHOOK(0), code, wparam, lparam);
            }
            let x = info.pt.x;
            let y = info.pt.y;
            let t = ENGINE.elapsed_ms();
            let msg = wparam.0 as u32;
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
                }),
                WM_LBUTTONDOWN => Some(mouse_btn("mouse_down", "left", t, x, y)),
                WM_LBUTTONUP => Some(mouse_btn("mouse_up", "left", t, x, y)),
                WM_RBUTTONDOWN => Some(mouse_btn("mouse_down", "right", t, x, y)),
                WM_RBUTTONUP => Some(mouse_btn("mouse_up", "right", t, x, y)),
                WM_MBUTTONDOWN => Some(mouse_btn("mouse_down", "middle", t, x, y)),
                WM_MBUTTONUP => Some(mouse_btn("mouse_up", "middle", t, x, y)),
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
                    })
                }
                _ => None,
            };
            if let Some(ev) = ev {
                ENGINE.push(ev);
            }
        }
        CallNextHookEx(HHOOK(0), code, wparam, lparam)
    }

    fn mouse_btn(kind: &str, button: &str, t: u64, x: i32, y: i32) -> InputEvent {
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
        }
    }

    unsafe extern "system" fn key_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && ENGINE.recording.load(Ordering::SeqCst) {
            let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            if (info.flags & 0x00000012) != 0 {
                return CallNextHookEx(HHOOK(0), code, wparam, lparam);
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
                });
            }
        }
        CallNextHookEx(HHOOK(0), code, wparam, lparam)
    }

    fn hook_loop() {
        unsafe {
            let hmod = GetModuleHandleW(None).unwrap_or_default();
            MOUSE_HOOK = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_proc),
                HINSTANCE(hmod.0),
                0,
            )
            .unwrap_or_default();
            KEY_HOOK = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(key_proc),
                HINSTANCE(hmod.0),
                0,
            )
            .unwrap_or_default();
            if MOUSE_HOOK.is_invalid() || KEY_HOOK.is_invalid() {
                if !MOUSE_HOOK.is_invalid() {
                    let _ = UnhookWindowsHookEx(MOUSE_HOOK);
                }
                if !KEY_HOOK.is_invalid() {
                    let _ = UnhookWindowsHookEx(KEY_HOOK);
                }
                MOUSE_HOOK = HHOOK(0);
                KEY_HOOK = HHOOK(0);
                return;
            }
            *HOOK_THREAD.lock().unwrap() = Some(GetCurrentThreadId());
            let mut msg = MSG::default();
            loop {
                let ret = GetMessageW(&mut msg, HWND(0), 0, 0).0;
                if ret == 0 || ret == -1 {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if !MOUSE_HOOK.is_invalid() {
                let _ = UnhookWindowsHookEx(MOUSE_HOOK);
            }
            if !KEY_HOOK.is_invalid() {
                let _ = UnhookWindowsHookEx(KEY_HOOK);
            }
            MOUSE_HOOK = HHOOK(0);
            KEY_HOOK = HHOOK(0);
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
        let left = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let sw = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let sh = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        let ax = if sw <= 1 { x } else { (x - left) * NORM / (sw - 1) };
        let ay = if sh <= 1 { y } else { (y - top) * NORM / (sh - 1) };
        (ax, ay)
    }

    fn send_mouse(x: i32, y: i32, flags: u32, mouse_data: i32) {
        const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;
        let (ax, ay) = abs_xy(x, y);
        let mut input = INPUT {
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
        let mut last = 0u64;
        for ev in events {
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
}
