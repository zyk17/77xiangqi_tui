//! 本地设置持久化（`key=value` 文本，无 JSON）。

use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "xiangqi_tui.conf";
const KEY_ENGINE_PATH: &str = "engine_path";

pub fn load_engine_path() -> String {
    if let Ok(value) = std::env::var("XIANGQI_ENGINE_PATH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    read_key(CONFIG_FILE, KEY_ENGINE_PATH).unwrap_or_default()
}

pub fn save_engine_path(path: &str) -> std::io::Result<()> {
    write_key(CONFIG_FILE, KEY_ENGINE_PATH, path.trim())
}

fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE)
}

fn read_key(path: &str, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    parse_key(&text, key)
}

fn write_key(path: &str, key: &str, value: &str) -> std::io::Result<()> {
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
}
