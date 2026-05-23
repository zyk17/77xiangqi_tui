use ratatui::{
    layout::Alignment,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::{
    app::{App, BattleButton, Focus, Screen, SettingsSection, TopTab},
    xiangqi::Board90,
};

#[derive(Debug, Clone, Copy)]
pub struct RenderOutput {
    pub board_area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    TopTab(TopTab),
    BattleButton(BattleButton),
    CommandInput,
    SettingsSection(SettingsSection),
    BoardCell(u8, u8),
}

const BUTTON_ROWS: [[Option<BattleButton>; 3]; 4] = [
    [Some(BattleButton::RedAi), Some(BattleButton::BlackAi), Some(BattleButton::QueryMode)],
    [Some(BattleButton::NewGame), Some(BattleButton::Undo), Some(BattleButton::RotateBoard)],
    [Some(BattleButton::PrevMove), Some(BattleButton::NextMove), Some(BattleButton::CopyFen)],
    [Some(BattleButton::PasteFen), Some(BattleButton::RealtimeEval), None],
];

pub fn render(frame: &mut Frame<'_>, app: &App) -> RenderOutput {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
        .split(frame.area());

    render_tabs(frame, root[0], app);

    let board_area = match app.screen {
        Screen::Battle => render_battle(frame, root[1], app),
        Screen::Settings => render_settings(frame, root[1], app),
    };

    frame.render_widget(
        Paragraph::new(app.status.as_str())
            .block(block("状态"))
            .wrap(Wrap { trim: true }),
        root[2],
    );

    RenderOutput { board_area }
}

pub fn hit_test(column: u16, row: u16, board_area: Option<Rect>) -> Option<HitTarget> {
    if row <= 2 {
        return if column < 40 {
            Some(HitTarget::TopTab(TopTab::Battle))
        } else {
            Some(HitTarget::TopTab(TopTab::Settings))
        };
    }

    if row >= 23 {
        return Some(HitTarget::CommandInput);
    }

    if (4..=15).contains(&row) && (68..=120).contains(&column) {
        let row_index = ((row - 4) / 3) as usize;
        let col_index = ((column - 68) / 17) as usize;
        return BUTTON_ROWS
            .get(row_index)
            .and_then(|button_row| button_row.get(col_index))
            .and_then(|button| *button)
            .map(HitTarget::BattleButton);
    }

    if (4..=16).contains(&row) && (4..=115).contains(&column) {
        return if row <= 10 {
            Some(HitTarget::SettingsSection(SettingsSection::Engine))
        } else {
            Some(HitTarget::SettingsSection(SettingsSection::OpeningBook))
        };
    }

    let area = board_area?;
    if column < area.x || column >= area.right() || row < area.y || row >= area.bottom() {
        return None;
    }

    let local_x = column.saturating_sub(area.x + 4);
    let local_y = row.saturating_sub(area.y + 2);
    let file = (local_x / 4).min(8) as u8;
    let rank = (local_y / 2).min(9) as u8;
    Some(HitTarget::BoardCell(file, rank))
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    for (index, tab) in TopTab::ALL.iter().enumerate() {
        let active = matches!(
            (tab, app.screen),
            (TopTab::Battle, Screen::Battle) | (TopTab::Settings, Screen::Settings)
        );
        let focused = app.focus == Focus::TopTab(*tab);
        let title = match tab {
            TopTab::Battle => "对弈",
            TopTab::Settings => "设置",
        };
        frame.render_widget(tab_widget(title, active, focused), chunks[index]);
    }
}

fn render_battle(frame: &mut Frame<'_>, area: Rect, app: &App) -> Rect {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(16), Constraint::Length(5)])
        .split(area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(68), Constraint::Min(42)])
        .split(rows[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(14), Constraint::Min(16)])
        .split(columns[1]);

    let board_area = render_board(frame, columns[0], &app.game.board, app.game.rotated);
    render_buttons(frame, right[0], app);
    render_eval_panel(frame, right[1], app);
    render_command_input(frame, rows[1], app);
    board_area
}

fn render_settings(frame: &mut Frame<'_>, area: Rect, app: &App) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Length(7), Constraint::Min(0)])
        .split(area);

    let engine_lines = vec![
        Line::from(format!("路径: {}", display_or_placeholder(&app.engine.path))),
        Line::from(format!("协议: {}", app.engine.protocol.label())),
        Line::from(format!("线程: {}", app.engine.threads)),
        Line::from(format!("Hash(MB): {}", app.engine.hash_mb)),
        Line::from(format!("Skill: {}", app.engine.skill_level)),
        Line::from(format!("MultiPV: {}", app.engine.multi_pv)),
    ];
    frame.render_widget(
        Paragraph::new(engine_lines)
            .block(section_block(
                "引擎设置",
                app.focus == Focus::SettingsSection(SettingsSection::Engine),
            ))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    let book_lines = vec![
        Line::from(format!("本地路径: {}", display_or_placeholder(&app.book.local_path))),
        Line::from(format!("启用本地库: {}", yes_no(app.book.local_enabled))),
        Line::from(format!("启用云库: {}", yes_no(app.book.cloud_enabled))),
        Line::from(format!("选取模式: {}", app.book.pick_mode)),
        Line::from(format!("最大步数: {}", app.book.max_halfmoves)),
    ];
    frame.render_widget(
        Paragraph::new(book_lines)
            .block(section_block(
                "开局库设置",
                app.focus == Focus::SettingsSection(SettingsSection::OpeningBook),
            ))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new("引擎流式调用、按钮字体与行为细节继续参考 GUI 仓库。")
            .block(block("说明"))
            .wrap(Wrap { trim: true }),
        chunks[2],
    );
    Rect::default()
}

fn render_board(frame: &mut Frame<'_>, area: Rect, board: &Board90, rotated: bool) -> Rect {
    let mut lines = Vec::with_capacity(24);
    lines.push(Line::from(grid_border('┌', '┬', '┐')));
    for rank in 0..10_u8 {
        let axis_rank = if rotated { rank } else { 9 - rank };
        let mut spans = vec![Span::styled(
            format!("{:>2}", axis_rank),
            Style::default().fg(Color::Gray),
        )];
        spans.push(Span::raw("│"));
        for file in 0..9_u8 {
            let display_rank = if rotated { rank } else { 9 - rank };
            let display_file = if rotated { 8 - file } else { file };
            let piece = board.display_at(display_file, display_rank);
            spans.push(Span::styled(format!(" {} ", piece), piece_style(piece)));
            if file != 8 {
                spans.push(Span::raw("│"));
            }
        }
        spans.push(Span::raw("│"));
        lines.push(Line::from(spans));
        if rank != 9 {
            lines.push(Line::from(grid_border('├', '┼', '┤')));
        }
    }
    lines.push(Line::from(grid_border('└', '┴', '┘')));
    lines.push(Line::from("    a   b   c   d   e   f   g   h   i"));
    frame.render_widget(
        Paragraph::new(lines)
            .block(block("A 棋盘"))
            .wrap(Wrap { trim: false }),
        area,
    );
    area
}

fn render_command_input(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let title = if app.focus == Focus::CommandInput {
        "C 输入*"
    } else {
        "C 输入"
    };
    let input_line = render_input_line(app);
    let mut input_lines = vec![input_line];
    let suggestions = app.input.suggestions();
    if !suggestions.is_empty() {
        let rendered = suggestions
            .into_iter()
            .take(3)
            .map(|command| format!("{} {}", command.name(), command.description()))
            .collect::<Vec<_>>()
            .join("   ");
        input_lines.push(Line::from(Span::styled(
            rendered,
            Style::default().fg(Color::LightYellow),
        )));
    }
    frame.render_widget(
        Paragraph::new(input_lines)
            .block(section_block(title, app.focus == Focus::CommandInput))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_buttons(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    for (row_area, row_buttons) in rows.iter().zip(BUTTON_ROWS.iter()) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(*row_area);
        for (index, button) in row_buttons.iter().enumerate() {
            match button {
                Some(button) => render_button(frame, cols[index], *button, app),
                None => frame.render_widget(Block::default(), cols[index]),
            }
        }
    }
}

fn render_button(frame: &mut Frame<'_>, area: Rect, button: BattleButton, app: &App) {
    let focused = app.focus == Focus::BattleButton(button);
    let active = match button {
        BattleButton::RedAi => app.game.red_ai,
        BattleButton::BlackAi => app.game.black_ai,
        BattleButton::QueryMode => app.game.query_mode,
        BattleButton::RealtimeEval => app.game.realtime_eval,
        _ => false,
    };
    frame.render_widget(button_widget(button.label(), active, focused), area);
}

fn render_eval_panel(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(5)])
        .split(area);

    let score_rows = vec![
        Row::new(vec![
            "用时".to_string(),
            "深度".to_string(),
            "NPS".to_string(),
            "节点".to_string(),
        ]),
        Row::new(vec![
            app.game.analysis.time_text.clone(),
            app.game.analysis.depth.to_string(),
            app.game.analysis.nps.to_string(),
            app.game.analysis.nodes.to_string(),
        ]),
        Row::new(vec![
            "分数".to_string(),
            "推荐".to_string(),
            "红/黑".to_string(),
            "".to_string(),
        ]),
        Row::new(vec![
            app.game.analysis.score_text.clone(),
            app.game.analysis.best_move.clone(),
            app.game.analysis.win_rate_text.clone(),
            "".to_string(),
        ]),
    ];
    let table = Table::new(
        score_rows,
        [
            Constraint::Percentage(24),
            Constraint::Percentage(24),
            Constraint::Percentage(24),
            Constraint::Percentage(28),
        ],
    )
    .block(block(&format!("D 实时评估 [{}]", app.game.analysis.source)))
    .column_spacing(1)
    .style(Style::default().fg(Color::Rgb(248, 242, 232)))
    .row_highlight_style(Style::default().fg(Color::White));
    frame.render_widget(table, sections[0]);

    let pv_lines = if app.game.analysis.pv.is_empty() {
        vec![Line::from("PV: --")]
    } else {
        app.game
            .analysis
            .pv
            .chunks(8)
            .take(2)
            .enumerate()
            .map(|(index, chunk)| {
                let prefix = if index == 0 { "PV1" } else { "PV2" };
                Line::from(format!("{prefix}: {}", chunk.join(" ")))
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(pv_lines)
            .block(block("PV 列表"))
            .wrap(Wrap { trim: true }),
        sections[1],
    );
}

fn tab_widget(title: &str, active: bool, focused: bool) -> Paragraph<'static> {
    Paragraph::new(title.to_string())
        .style(if active {
            Style::default().fg(Color::Black).bg(Color::Rgb(236, 221, 203))
        } else {
            Style::default().fg(Color::DarkGray)
        })
        .block(section_block(title, focused))
}

fn button_widget(title: &str, active: bool, focused: bool) -> Paragraph<'static> {
    Paragraph::new(title.to_string())
        .alignment(Alignment::Center)
        .style(if active {
            Style::default()
                .fg(Color::Rgb(48, 32, 24))
                .bg(Color::Rgb(223, 210, 197))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(236, 228, 214))
        })
        .block(button_block(active, focused))
}

fn block(title: &str) -> Block<'static> {
    Block::default().borders(Borders::ALL).title(title.to_string())
}

fn section_block(title: &str, focused: bool) -> Block<'static> {
    Block::default().borders(Borders::ALL).title(Span::styled(
        title.to_string(),
        if focused {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        },
    ))
}

fn button_block(active: bool, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if active {
        Style::default().fg(Color::Rgb(210, 153, 96))
    } else {
        Style::default().fg(Color::Rgb(160, 142, 124))
    };
    Block::default().borders(Borders::ALL).border_style(border_style)
}

fn render_input_line(app: &App) -> Line<'static> {
    let prompt = "❯ ";
    let text = app.input.text();
    let cursor = app.input.cursor();
    let before = &text[..cursor];
    let after = &text[cursor..];
    let current = after.chars().next();

    let mut spans = vec![Span::styled(
        prompt.to_string(),
        Style::default().fg(Color::Rgb(255, 190, 120)).add_modifier(Modifier::BOLD),
    )];
    spans.push(Span::raw(before.to_string()));
    match current {
        Some(ch) => {
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(Color::Black).bg(Color::White),
            ));
            spans.push(Span::raw(after[ch.len_utf8()..].to_string()));
        }
        None => {
            spans.push(Span::styled(
                " ".to_string(),
                Style::default().bg(Color::White),
            ));
        }
    }
    Line::from(spans)
}

fn grid_border(left: char, middle: char, right: char) -> String {
    let mut line = String::from("  ");
    line.push(left);
    for file in 0..9 {
        line.push_str("───");
        if file == 8 {
            line.push(right);
        } else {
            line.push(middle);
        }
    }
    line
}

fn piece_style(piece: char) -> Style {
    if piece.is_ascii_uppercase() {
        Style::default().fg(Color::Red)
    } else if piece.is_ascii_lowercase() {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::Gray)
    }
}

fn display_or_placeholder(value: &str) -> String {
    if value.is_empty() {
        "<未设置>".to_string()
    } else {
        value.to_string()
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}
