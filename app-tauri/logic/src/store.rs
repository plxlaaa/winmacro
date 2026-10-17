//! JSON persistence in `~/.winmacro/`, byte-compatible with the legacy
//! Python edition. Writes are atomic (tmp + rename).

use std::fs;
use std::io;
use std::path::PathBuf;

use crate::model::{Recording, Settings};

pub fn home_dir() -> Result<PathBuf, String> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| "无法定位用户目录".to_string())
}

pub fn data_dir() -> Result<PathBuf, String> {
    let dir = home_dir()?.join(".winmacro");
    fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败：{e}"))?;
    Ok(dir)
}

fn recordings_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("recordings.json"))
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("settings.json"))
}

fn atomic_write(path: &PathBuf, text: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text).map_err(|e| format!("写入失败：{e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("替换文件失败：{e}"))?;
    Ok(())
}

pub fn load_recordings() -> Vec<Recording> {
    let path = match recordings_path() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    // Legacy layout is a bare list; tolerate a {"recordings": [...]} wrapper too.
    let parsed: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let items = match &parsed {
        serde_json::Value::Array(_) => parsed,
        serde_json::Value::Object(_) => parsed["recordings"].clone(),
        _ => serde_json::Value::Array(Vec::new()),
    };
    serde_json::from_value::<Vec<Recording>>(items).unwrap_or_default()
}

pub fn save_recordings(recordings: &[Recording]) -> Result<(), String> {
    let path = recordings_path()?;
    let text = serde_json::to_string_pretty(recordings).map_err(|e| e.to_string())?;
    atomic_write(&path, &text)
}

pub fn load_settings() -> Settings {
    let path = match settings_path() {
        Ok(p) => p,
        Err(_) => return Settings::default(),
    };
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Settings::default(),
    };
    serde_json::from_str::<Settings>(&text)
        .unwrap_or_default()
        .sanitize()
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    let clean = settings.clone().sanitize();
    let text = serde_json::to_string_pretty(&clean).map_err(|e| e.to_string())?;
    atomic_write(&path, &text)
}

pub fn export_recording(recording: &Recording, path: &PathBuf) -> Result<(), String> {
    let text = serde_json::to_string_pretty(recording).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("导出失败：{e}"))
}

pub fn import_recording(path: &PathBuf) -> Result<Recording, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("读取失败：{e}"))?;
    let rec: Recording = serde_json::from_str(&text).map_err(|e| format!("文件格式错误：{e}"))?;
    if rec.events.is_empty() {
        return Err("脚本没有任何步骤".into());
    }
    Ok(rec)
}

pub fn ensure_unique_id(mut rec: Recording, existing: &[Recording]) -> Recording {
    let taken = |id: &str| existing.iter().any(|r| r.id == id);
    if taken(&rec.id) {
        loop {
            rec.id = crate::model::new_id();
            if !taken(&rec.id) {
                break;
            }
        }
    }
    rec
}

/// Disk usage of the data directory (bytes) — surfaced in Settings.
pub fn data_size() -> u64 {
    fn dir_size(p: &PathBuf) -> u64 {
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(p) {
            for entry in entries.flatten() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        total += dir_size(&entry.path());
                    }
                }
            }
        }
        total
    }
    let _ = io::empty();
    recordings_path()
        .ok()
        .and_then(|p| p.parent().map(|d| dir_size(&d.to_path_buf())))
        .unwrap_or(0)
}
