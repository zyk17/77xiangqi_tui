//! 操作说明全屏浮层（`/help`，Esc 关闭）。

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::style::{highlight, text as text_style, text_bold};

pub fn render_help_overlay(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Clear, area);
    let popup = centered_rect(88, 90, area);
    let lines = help_lines();
    frame.render_widget(
        Paragraph::new(lines)
            .style(text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(
                        " 操作说明 — Esc 关闭 ",
                        text_bold().add_modifier(Modifier::BOLD),
                    )),
            )
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left),
        popup,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let width = area.width.saturating_mul(percent_x) / 100;
    let height = area.height.saturating_mul(percent_y) / 100;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.max(1), height.max(1))
}

fn help_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled("版面", text_bold())),
        Line::from("  A 棋盘  B 按钮  C 命令输入  D 实时评估（含 PV）"),
        Line::from(""),
        Line::from(Span::styled("切换", text_bold())),
        Line::from("  Tab：对弈 ↔ 设置（输入框内 Tab 为命令补全）"),
        Line::from("  顶栏 Tab：鼠标点击「对弈 / 设置」"),
        Line::from(""),
        Line::from(Span::styled("棋盘（对弈页，焦点在棋盘时）", text_bold())),
        Line::from("  方向键：移动光标（全局 UCI 坐标 a–i / 0–9）"),
        Line::from("  空格：选子 / 落子（再次点己方棋子可改选）"),
        Line::from("  : 进入命令输入；/ 打开命令列表"),
        Line::from("  ? 打开本帮助"),
        Line::from(""),
        Line::from(Span::styled("C 区命令输入", text_bold())),
        Line::from("  Enter：执行；Esc：返回棋盘"),
        Line::from("  ↑↓：翻阅已执行命令历史（未 Enter 不入栈，可改字）"),
        Line::from("  Tab / →：补全 / 命令；↑↓：在 / 菜单中换选"),
        Line::from("  普通着法：四字符 UCI，如 h2e2"),
        Line::from(""),
        Line::from(Span::styled("设置页", text_bold())),
        Line::from("  ↑↓：选择设置项；Enter：在 C 区编辑或切换开关"),
        Line::from("  ←→ / 空格：调整协议、数值、开关"),
        Line::from(""),
        Line::from(Span::styled("命令", text_bold())),
        Line::from("  /stop     停止模式、引擎流与自动走子（局面不变）"),
        Line::from("  /new      先 /stop 逻辑，再开一把新棋"),
        Line::from("  /undo     悔棋"),
        Line::from("  /prev /next  浏览历史步"),
        Line::from("  /rai /bai 红/黑电脑开关"),
        Line::from("  /query    查询模式"),
        Line::from("  /eval     实时评估"),
        Line::from("  /rotate   旋转棋盘"),
        Line::from("  /copyfen  复制 FEN 到剪贴板"),
        Line::from("  /pastefen <FEN>  载入局面"),
        Line::from("  /help     本说明"),
        Line::from("  /exit /quit  退出"),
        Line::from(""),
        Line::from(Span::styled("按 Esc 返回。", highlight())),
    ]
}
