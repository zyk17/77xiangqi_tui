//! 本地设置持久化（`key=value` 文本，无 JSON）。

use std::fs;
use std::path::PathBuf;

use crate::engine::EngineProtocol;

const CONFIG_FILE: &str = "xiangqi_tui.conf";
const KEY_ENGINE_PATH: &str = "engine_path";
const KEY_ENGINE_PROTOCOL: &str = "engine_protocol";
const KEY_ENGINE_THREADS: &str = "engine_threads";
const KEY_ENGINE_HASH_MB: &str = "engine_hash_mb";
const KEY_ENGINE_SKILL: &str = "engine_skill";
const KEY_ENGINE_MULTI_PV: &str = "engine_multi_pv";
const KEY_BOOK_LOCAL_PATH: &str = "book_local_path";
const KEY_BOOK_LOCAL_ENABLED: &str = "book_local_enabled";
const KEY_BOOK_CLOUD_ENABLED: &str = "book_cloud_enabled";
const KEY_BOOK_PICK_MODE: &str = "book_pick_mode";
const KEY_BOOK_MAX_HALFMOVES: &str = "book_max_halfmoves";

pub fn load_engine_path() -> String {
    if let Ok(value) = std::env::var("XIANGQI_ENGINE_PATH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    read_key(CONFIG_FILE, KEY_ENGINE_PATH).unwrap_or_default()
}

pub fn load_engine_protocol() -> EngineProtocol {
    match read_key(CONFIG_FILE, KEY_ENGINE_PROTOCOL)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "ucci" => EngineProtocol::Ucci,
        _ => EngineProtocol::Uci,
    }
}

pub fn load_engine_threads() -> u8 {
    read_key(CONFIG_FILE, KEY_ENGINE_THREADS)
        .and_then(|v| v.parse().ok())
        .unwrap_or(4)
        .clamp(1, 64)
}

pub fn load_engine_hash_mb() -> u32 {
    read_key(CONFIG_FILE, KEY_ENGINE_HASH_MB)
        .and_then(|v| v.parse().ok())
        .unwrap_or(512)
        .clamp(64, 8192)
}

pub fn load_engine_skill() -> u8 {
    read_key(CONFIG_FILE, KEY_ENGINE_SKILL)
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
        .min(20)
}

pub fn load_engine_multi_pv() -> u8 {
    read_key(CONFIG_FILE, KEY_ENGINE_MULTI_PV)
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .clamp(1, 5)
}

pub fn load_book_local_path() -> String {
    read_key(CONFIG_FILE, KEY_BOOK_LOCAL_PATH).unwrap_or_default()
}

pub fn load_book_local_enabled() -> bool {
    read_key(CONFIG_FILE, KEY_BOOK_LOCAL_ENABLED)
        .map(|v| parse_bool(&v))
        .unwrap_or(true)
}

pub fn load_book_cloud_enabled() -> bool {
    read_key(CONFIG_FILE, KEY_BOOK_CLOUD_ENABLED)
        .map(|v| parse_bool(&v))
        .unwrap_or(false)
}

fn normalize_book_pick_mode(mode: &str) -> String {
    if mode == "positive_random" {
        "positive_random".to_string()
    } else {
        "optimal".to_string()
    }
}

pub fn load_book_pick_mode() -> String {
    let mode = read_key(CONFIG_FILE, KEY_BOOK_PICK_MODE).unwrap_or_else(|| "optimal".to_string());
    normalize_book_pick_mode(&mode)
}

pub fn load_book_max_halfmoves() -> u16 {
    read_key(CONFIG_FILE, KEY_BOOK_MAX_HALFMOVES)
        .and_then(|v| v.parse().ok())
        .unwrap_or(999)
}

pub fn save_engine_path(path: &str) -> std::io::Result<()> {
    write_key(CONFIG_FILE, KEY_ENGINE_PATH, path.trim())
}

pub fn save_engine_protocol(protocol: EngineProtocol) -> std::io::Result<()> {
    let value = match protocol {
        EngineProtocol::Uci => "uci",
        EngineProtocol::Ucci => "ucci",
    };
    write_key(CONFIG_FILE, KEY_ENGINE_PROTOCOL, value)
}

pub fn save_engine_threads(threads: u8) -> std::io::Result<()> {
    write_key(CONFIG_FILE, KEY_ENGINE_THREADS, &threads.to_string())
}

pub fn save_engine_hash_mb(hash_mb: u32) -> std::io::Result<()> {
    write_key(CONFIG_FILE, KEY_ENGINE_HASH_MB, &hash_mb.to_string())
}

pub fn save_engine_skill(skill: u8) -> std::io::Result<()> {
    write_key(CONFIG_FILE, KEY_ENGINE_SKILL, &skill.to_string())
}

pub fn save_engine_multi_pv(multi_pv: u8) -> std::io::Result<()> {
    write_key(CONFIG_FILE, KEY_ENGINE_MULTI_PV, &multi_pv.to_string())
}

pub fn save_book_local_path(path: &str) -> std::io::Result<()> {
    write_key(CONFIG_FILE, KEY_BOOK_LOCAL_PATH, path.trim())
}

pub fn save_book_flags(local_enabled: bool, cloud_enabled: bool) -> std::io::Result<()> {
    write_key(
        CONFIG_FILE,
        KEY_BOOK_LOCAL_ENABLED,
        if local_enabled { "1" } else { "0" },
    )?;
    write_key(
        CONFIG_FILE,
        KEY_BOOK_CLOUD_ENABLED,
        if cloud_enabled { "1" } else { "0" },
    )
}

pub fn save_book_pick_mode(mode: &str) -> std::io::Result<()> {
    let mode = if mode == "positive_random" {
        "positive_random"
    } else {
        "optimal"
    };
    write_key(CONFIG_FILE, KEY_BOOK_PICK_MODE, mode)
}

pub fn save_book_max_halfmoves(max: u16) -> std::io::Result<()> {
    write_key(CONFIG_FILE, KEY_BOOK_MAX_HALFMOVES, &max.to_string())
}

fn parse_bool(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE)
}

fn read_key(path: &str, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    parse_key(&text, key)
}

fn write_key(_path: &str, key: &str, value: &str) -> std::io::Result<()> {
    let file = config_path();
    let mut lines: Vec<String> = if file.exists() {
        fs::read_to_string(&file)?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_string)
            .collect()
    } else {
        Vec::new()
    };
    set_line(&mut lines, key, value);
    let body = if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    };
    fs::write(file, body)
}

fn parse_key(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&prefix).map(str::to_string))
}

fn set_line(lines: &mut Vec<String>, key: &str, value: &str) {
    let prefix = format!("{key}=");
    if let Some(index) = lines.iter().position(|line| line.trim().starts_with(&prefix)) {
        lines[index] = format!("{key}={value}");
    } else {
        lines.push(format!("{key}={value}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_set_roundtrip() {
        let mut lines = vec!["book_path=abc".to_string()];
        set_line(&mut lines, "engine_path", r"C:\eng.exe");
        let text = lines.join("\n");
        assert_eq!(
            parse_key(&text, "engine_path").as_deref(),
            Some(r"C:\eng.exe")
        );
    }

    #[test]
    fn pick_mode_normalizes() {
        assert_eq!(normalize_book_pick_mode("optimal"), "optimal");
        assert_eq!(normalize_book_pick_mode("positive_random"), "positive_random");
        assert_eq!(normalize_book_pick_mode("other"), "optimal");
    }
}
