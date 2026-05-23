//! 系统剪贴板（复制 FEN 等）。

pub fn copy_text(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .map_err(|e| format!("无法打开剪贴板：{e}"))?
        .set_text(text)
        .map_err(|e| format!("复制失败：{e}"))
}
