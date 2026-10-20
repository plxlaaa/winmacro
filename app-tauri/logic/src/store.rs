//! Persistence in `~/.winmacro/`:
//! - **Settings**: `settings.json` (JSON, human-readable for manual edits)
//! - **Recordings**: `recordings.bin` (binary, fast load + smaller) with
//!   automatic migration from legacy `recordings.json`
//!
//! Atomic writes everywhere (tmp + rename) to avoid partial-file corruption
//! on crash.

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
    data_dir_at(&home_dir()?)
}

fn data_dir_at(base: &std::path::Path) -> Result<PathBuf, String> {
    let dir = base.join(".winmacro");
    fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败：{e}"))?;
    Ok(dir)
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("settings.json"))
}

/// 文件头魔数：`WMAC` + 版本字节。
const MAGIC: &[u8; 4] = b"WMAC";
const VERSION: u8 = 1;

fn atomic_write_bytes(path: &PathBuf, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("写入失败：{e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("替换文件失败：{e}"))?;
    Ok(())
}

fn atomic_write(path: &PathBuf, text: &str) -> Result<(), String> {
    atomic_write_bytes(path, text.as_bytes())
}

/// 把 Recording 列表序列化为二进制：5 字节魔数 + bincode payload。
fn encode_bincode(records: &[Recording]) -> Result<Vec<u8>, String> {
    let payload = bincode::serialize(records).map_err(|e| format!("序列化失败：{e}"))?;
    let mut buf = Vec::with_capacity(5 + payload.len());
    buf.extend_from_slice(MAGIC);
    buf.push(VERSION);
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// 解析二进制格式。版本不匹配、魔数错误 → Err。
fn decode_bincode(bytes: &[u8]) -> Result<Vec<Recording>, String> {
    if bytes.len() < 5 {
        return Err("文件过短".into());
    }
    if &bytes[0..4] != MAGIC {
        return Err("文件魔数不匹配".into());
    }
    let ver = bytes[4];
    if ver != VERSION {
        return Err(format!("不支持的版本：{ver}"));
    }
    bincode::deserialize(&bytes[5..]).map_err(|e| format!("反序列化失败：{e}"))
}

/// 加载录制列表。优先读 BIN，缺失/损坏时回退到 JSON，并自动迁移。
pub fn load_recordings() -> Vec<Recording> {
    match home_dir() {
        Ok(base) => load_recordings_at(&base.join(".winmacro")),
        Err(_) => Vec::new(),
    }
}

fn load_recordings_at(dir: &std::path::Path) -> Vec<Recording> {
    let bin_path = dir.join("recordings.bin");
    let json_path = dir.join("recordings.json");

    // 1. 优先尝试 BIN
    if let Ok(bytes) = fs::read(&bin_path) {
        match decode_bincode(&bytes) {
            Ok(records) => return records,
            Err(e) => eprintln!("[winmacro] 录制文件损坏，回退到 JSON：{e}"),
        }
    }
    // 2. 回退到 JSON（兼容 v1 / Python 旧版）
    let text = match fs::read_to_string(&json_path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let parsed: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let items = match &parsed {
        serde_json::Value::Array(_) => parsed,
        serde_json::Value::Object(_) => parsed["recordings"].clone(),
        _ => serde_json::Value::Array(Vec::new()),
    };
    let records: Vec<Recording> = serde_json::from_value(items).unwrap_or_default();

    // 3. BIN 写入成功后才归档 JSON；磁盘满或权限不足时保留原始数据。
    if !records.is_empty() {
        if let Ok(bytes) = encode_bincode(&records) {
            if atomic_write_bytes(&bin_path, &bytes).is_ok() {
                let _ = fs::rename(&json_path, json_path.with_extension("json.bak"));
            }
        }
    }
    records
}

/// 保存录制列表到 BIN。旧的 JSON 文件若存在会保留为 .bak（一次性迁移在 load 时已完成）。
pub fn save_recordings(records: &[Recording]) -> Result<(), String> {
    let dir = data_dir()?;
    let path = dir.join("recordings.bin");
    let bytes = encode_bincode(records)?;
    atomic_write_bytes(&path, &bytes)
}

/// 调试用：探测当前存储格式。
pub fn storage_format() -> StorageFormat {
    let dir = match data_dir() {
        Ok(d) => d,
        Err(_) => return StorageFormat::None,
    };
    if dir.join("recordings.bin").exists() {
        return StorageFormat::Bincode;
    }
    if dir.join("recordings.json").exists() {
        return StorageFormat::Json;
    }
    StorageFormat::None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageFormat {
    None,
    Json,
    Bincode,
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

/// 导出单个 Recording 仍用 JSON（用户能直接打开看）。
pub fn export_recording(recording: &Recording, path: &PathBuf) -> Result<(), String> {
    let text = serde_json::to_string_pretty(recording).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("导出失败：{e}"))
}

/// 导入也只接受 JSON（跨工具的通用格式）。
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
    data_dir()
        .ok()
        .map(|d| dir_size(&d))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{InputEvent, Recording};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// 临时目录（每个测试独立，直接传路径给 _at 变体）。
    /// 注意：之前的版本用 env::set_var 设 USERPROFILE，Rust 1.79+ 该
    /// API 被标 unsafe（多线程下设环境变量是 UB），且 set_var 在测试
    /// 期间未必生效；现在直接传路径，干净可靠。
    fn with_tmp<F: FnOnce(&PathBuf)>(name: &str, f: F) {
        static SEQ: AtomicU32 = AtomicU32::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("winmacro-test-{name}-{pid}-{n}"));
        fs::create_dir_all(&dir).unwrap();
        f(&dir);
        let _ = fs::remove_dir_all(&dir);
    }

    fn fake_recording(name: &str) -> Recording {
        Recording {
            id: format!("test-{name}"),
            name: name.into(),
            created_at: "2026-10-18T12:00:00".into(),
            duration_ms: 1500,
            events: vec![
                InputEvent {
                    kind: "mouse_move".into(),
                    t_ms: 0,
                    x: Some(10),
                    y: Some(20),
                    ..Default::default()
                },
                InputEvent {
                    kind: "mouse_down".into(),
                    t_ms: 50,
                    x: Some(10),
                    y: Some(20),
                    button: Some("left".into()),
                    ..Default::default()
                },
                InputEvent {
                    kind: "key_down".into(),
                    t_ms: 100,
                    vk: Some(0x41),
                    key: Some("VK_41".into()),
                    ..Default::default()
                },
            ],
            screen_width: 1920,
            screen_height: 1080,
            resolution_mode: "auto".into(),
            hotkey: None,
        }
    }

    #[test]
    fn bincode_roundtrip_preserves_data() {
        with_tmp("bin-rt", |dir| {
            let recs = vec![fake_recording("a"), fake_recording("b")];
            // 写到指定目录
            let bytes = encode_bincode(&recs).unwrap();
            atomic_write_bytes(&dir.join("recordings.bin"), &bytes).unwrap();

            let loaded = load_recordings_at(dir);
            assert_eq!(loaded.len(), 2);
            assert_eq!(loaded[0].id, "test-a");
            assert_eq!(loaded[0].events.len(), 3);
            assert_eq!(loaded[0].events[2].vk, Some(0x41));
        });
    }

    #[test]
    fn json_to_bincode_migration_works() {
        with_tmp("migrate", |dir| {
            // 模拟旧版本：手工写一个 JSON
            let rec = fake_recording("legacy");
            let json = serde_json::to_string_pretty(&vec![rec]).unwrap();
            fs::write(dir.join("recordings.json"), &json).unwrap();

            // 第一次 load：应当读 JSON + 触发迁移
            let loaded = load_recordings_at(dir);
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded[0].id, "test-legacy");

            // 迁移后：BIN 文件应当存在，JSON 应当被重命名
            assert!(dir.join("recordings.bin").exists());
            assert!(!dir.join("recordings.json").exists());
            assert!(dir.join("recordings.json.bak").exists());

            // 第二次 load：走 BIN
            let loaded2 = load_recordings_at(dir);
            assert_eq!(loaded2.len(), 1);
            assert_eq!(loaded2[0].events.len(), 3);
        });
    }

    #[test]
    fn failed_migration_preserves_json_for_next_start() {
        with_tmp("migration-failed", |dir| {
            let json = serde_json::to_string(&vec![fake_recording("recoverable")]).unwrap();
            fs::write(dir.join("recordings.json"), &json).unwrap();
            // A directory at the staging path deterministically prevents writing.
            fs::create_dir(dir.join("recordings.tmp")).unwrap();
            for _ in 0..2 {
                let loaded = load_recordings_at(dir);
                assert_eq!(loaded[0].id, "test-recoverable");
                assert_eq!(fs::read_to_string(dir.join("recordings.json")).unwrap(), json);
                assert!(!dir.join("recordings.json.bak").exists());
            }
            fs::remove_dir(dir.join("recordings.tmp")).unwrap();
            assert_eq!(load_recordings_at(dir).len(), 1);
            assert!(dir.join("recordings.bin").exists());
            assert!(dir.join("recordings.json.bak").exists());
        });
    }

    #[test]
    fn empty_json_does_not_migrate() {
        with_tmp("empty", |dir| {
            fs::write(dir.join("recordings.json"), "[]").unwrap();
            let loaded = load_recordings_at(dir);
            assert!(loaded.is_empty());
            // 空 JSON 不触发迁移（避免无意义文件）
            assert!(!dir.join("recordings.bin").exists());
            assert!(dir.join("recordings.json").exists());
        });
    }

    #[test]
    fn corrupted_bincode_falls_back_to_json() {
        with_tmp("corrupt", |dir| {
            // 写一个损坏的 BIN
            fs::write(dir.join("recordings.bin"), b"WMAC\x01not-valid-bincode").unwrap();
            // 写一个好的 JSON 作为后备
            let rec = fake_recording("from-json");
            let json = serde_json::to_string_pretty(&vec![rec]).unwrap();
            fs::write(dir.join("recordings.json"), &json).unwrap();

            let loaded = load_recordings_at(dir);
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded[0].id, "test-from-json");
        });
    }

    #[test]
    fn wrong_magic_does_not_crash() {
        with_tmp("wrong-magic", |dir| {
            fs::write(dir.join("recordings.bin"), b"OTHER\x01valid-bincode").unwrap();
            // 没有 JSON 后备
            let loaded = load_recordings_at(dir);
            assert!(loaded.is_empty());
        });
    }

    #[test]
    fn future_version_is_rejected() {
        with_tmp("future", |dir| {
            fs::write(dir.join("recordings.bin"), b"WMAC\xFFpayload").unwrap();
            let loaded = load_recordings_at(dir);
            assert!(loaded.is_empty());
        });
    }

    #[test]
    fn empty_store_returns_empty_vec() {
        with_tmp("none", |dir| {
            assert_eq!(load_recordings_at(dir).len(), 0);
        });
    }

    #[test]
    fn bincode_is_smaller_than_json() {
        with_tmp("size", |_dir| {
            // 构造 50 个录制的集合
            let recs: Vec<Recording> = (0..50)
                .map(|i| {
                    let mut r = fake_recording(&format!("r{i}"));
                    // 每条录制 100 个事件
                    r.events = (0..100)
                        .map(|j| InputEvent {
                            kind: "mouse_move".into(),
                            t_ms: j as u64 * 10,
                            x: Some(j as i32),
                            y: Some(j as i32 * 2),
                            ..Default::default()
                        })
                        .collect();
                    r
                })
                .collect();

            let bin = encode_bincode(&recs).unwrap();
            let json = serde_json::to_string_pretty(&recs).unwrap();
            eprintln!("bin: {} bytes, json: {} bytes", bin.len(), json.len());
            // bincode 应该明显小于 JSON
            assert!(
                bin.len() < json.len() / 2,
                "bincode should be at least 2x smaller than JSON"
            );
        });
    }
}
