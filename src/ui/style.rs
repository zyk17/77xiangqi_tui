//! 终端主题无关样式：正文用默认前景色，强调用标准 ANSI 色（浅/深背景均可读）。

use ratatui::style::{Color, Modifier, Style};

/// 正文（随终端浅色/深色主题自动适配）
#[inline]
pub fn text() -> Style {
    Style::default()
}

/// 次要文字（略弱化，仍依赖终端默认色）
#[inline]
pub fn text_dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

#[inline]
pub fn text_bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

#[inline]
pub fn accent() -> Style {
    Style::default().fg(Color::Cyan)
}

#[inline]
pub fn highlight() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub fn active_flag() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub fn border_normal() -> Style {
    Style::default().fg(Color::DarkGray)
}

#[inline]
pub fn border_focused() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

#[inline]
pub fn border_active() -> Style {
    Style::default().fg(Color::Cyan)
}

/// 按钮：未按下（加粗，避免细字体在浅色终端上过淡）
#[inline]
pub fn button_idle() -> Style {
    text_bold()
}

/// 按钮：功能已开启
#[inline]
pub fn button_on() -> Style {
    active_flag()
}

/// 按钮：不可用（与 GUI `:disabled` 一致）
#[inline]
pub fn button_disabled() -> Style {
    text_dim()
}

/// 标签页：当前页
#[inline]
pub fn tab_active() -> Style {
    highlight()
}

/// 标签页：非当前
#[inline]
pub fn tab_idle() -> Style {
    text()
}

/// 红方棋子（深浅主题均易辨认）
#[inline]
pub fn piece_red() -> Style {
    Style::default().fg(Color::Red)
}

/// 黑方棋子
#[inline]
pub fn piece_black() -> Style {
    Style::default().fg(Color::Blue)
}

/// 输入提示符
#[inline]
pub fn input_prompt() -> Style {
    accent()
}

/// 命令补全提示
#[inline]
pub fn suggestion() -> Style {
    Style::default().fg(Color::Yellow)
}

/// 输入光标（反色块，深浅终端均可见）
#[inline]
pub fn cursor_cell() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}
