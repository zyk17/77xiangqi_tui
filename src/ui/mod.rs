mod board;
mod hit;
mod layout;
mod style;

use ratatui::{
    layout::Alignment,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
    Frame,
};

use self::style::{
    accent, active_flag, border_active, border_focused, border_normal, button_idle, button_on,
    cursor_cell, highlight, input_prompt, suggestion, tab_active, tab_idle,
    text as text_style, text_bold, text_dim,
};

use crate::app::{App, BattleButton, Focus, Screen, SettingsSection, TopTab};

pub use layout::UiRegions;

#[derive(Debug, Clone, Copy)]
pub struct RenderOutput {
    pub regions: UiRegions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    TopTab(TopTab),
    BattleButton(BattleButton),
    CommandInput,
    SettingsSection(SettingsSection),
    BoardCell(u8, u8),
}

/// 每个按钮块 3 行高（上下边框 + 文字），共 4 行。
const BUTTON_ROW_HEIGHT: u16 = 3;
const BUTTON_ROW_COUNT: u16 = 4;
const BUTTON_PANEL_HEIGHT: u16 = BUTTON_ROW_HEIGHT * BUTTON_ROW_COUNT;

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

    let tabs = render_tabs(frame, root[0], app);

    let screen = match app.screen {
        Screen::Battle => layout::ScreenRegions::Battle(render_battle(frame, root[1], app)),
        Screen::Settings => layout::ScreenRegions::Settings(render_settings(frame, root[1], app)),
    };

    frame.render_widget(
        Paragraph::new(app.status.as_str())
            .style(text_style())
            .block(block("状态"))
            .wrap(Wrap { trim: true }),
        root[2],
    );

    RenderOutput {
        regions: UiRegions { tabs, screen },
    }
}

pub fn hit_test(
    column: u16,
    row: u16,
    screen: Screen,
    regions: &UiRegions,
) -> Option<HitTarget> {
    hit::hit_test(column, row, screen, regions)
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, app: &App) -> layout::TabRegions {
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
    layout::TabRegions {
        battle: chunks[0],
        settings: chunks[1],
    }
}

fn render_battle(frame: &mut Frame<'_>, area: Rect, app: &App) -> layout::BattleRegions {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(area);

    // 棋盘占对弈区大部分宽度（接近用户红框），右侧按钮/评估。
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Min(34)])
        .split(rows[0]);
    let board_area = columns[0];
    // 按钮区固定 12 行；D 区占剩余。勿用 Min(16) 挤压按钮区，否则评估面板会盖住第 3、4 行按钮。
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(BUTTON_PANEL_HEIGHT), Constraint::Min(0)])
        .split(columns[1]);

    crate::runtime_log::debug(format!(
        "battle_layout col1={:?} buttons={:?} eval={:?}",
        columns[1], right[0], right[1]
    ));

    let board = board::render_grid_board(
        frame,
        board_area,
        &app.game.board,
        app.game.rotated,
        app.game.last_move_arrow,
        app.game.pending_arrow,
        app.game.selected_cell,
    );
    let (buttons, button_count) = render_buttons(frame, right[0], app);
    render_eval_panel(frame, right[1], app);
    render_command_input(frame, rows[1], app);
    layout::BattleRegions {
        board,
        board_rotated: app.game.rotated,
        command_input: rows[1],
        buttons,
        button_count,
    }
}

fn render_settings(frame: &mut Frame<'_>, area: Rect, app: &App) -> layout::SettingsRegions {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Length(7), Constraint::Min(0)])
        .split(rows[0]);

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
            .style(text_style())
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
            .style(text_style())
            .block(section_block(
                "开局库设置",
                app.focus == Focus::SettingsSection(SettingsSection::OpeningBook),
            ))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );

    let hint = if app.focus == Focus::SettingsSection(SettingsSection::Engine) {
        "下方 C 区输入引擎可执行文件路径，Enter 保存到 xiangqi_tui.conf。"
    } else {
        "参考 GUI 仓库实现细节；逻辑对照见 NextStep.md。"
    };
    frame.render_widget(
        Paragraph::new(hint)
            .style(text_style())
            .block(block("说明"))
            .wrap(Wrap { trim: true }),
        chunks[2],
    );
    render_command_input(frame, rows[1], app);
    layout::SettingsRegions {
        engine: chunks[0],
        book: chunks[1],
        command_input: rows[1],
    }
}

fn render_command_input(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let settings_engine_edit = app.screen == Screen::Settings
        && app.focus == Focus::SettingsSection(SettingsSection::Engine);
    let title = if settings_engine_edit {
        "C 引擎路径*"
    } else if app.focus == Focus::CommandInput {
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
        input_lines.push(Line::from(Span::styled(rendered, suggestion())));
    }
    frame.render_widget(
        Paragraph::new(input_lines)
            .style(text_style())
            .block(section_block(title, app.focus == Focus::CommandInput))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_buttons(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
) -> ([(BattleButton, Rect); 11], usize) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(BUTTON_ROW_HEIGHT),
            Constraint::Length(BUTTON_ROW_HEIGHT),
            Constraint::Length(BUTTON_ROW_HEIGHT),
            Constraint::Length(BUTTON_ROW_HEIGHT),
        ])
        .split(area);

    let mut buttons = [(BattleButton::NewGame, Rect::default()); 11];
    let mut button_count = 0usize;

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
                Some(button) => {
                    render_button(frame, cols[index], *button, app);
                    if button_count < buttons.len() {
                        buttons[button_count] = (*button, cols[index]);
                        button_count += 1;
                    }
                }
                None => frame.render_widget(Block::default(), cols[index]),
            }
        }
    }
    (buttons, button_count)
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
        .constraints([Constraint::Min(7), Constraint::Min(3)])
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
    .style(text_style())
    .row_highlight_style(highlight());
    frame.render_widget(table, sections[0]);

    let pv_lines = if app.game.analysis.pv.is_empty() {
        vec![Line::from(Span::styled("PV: --", text_dim()))]
    } else {
        app.game
            .analysis
            .pv
            .chunks(8)
            .take(2)
            .enumerate()
            .map(|(index, chunk)| {
                let prefix = if index == 0 { "PV1" } else { "PV2" };
                Line::from(Span::styled(
                    format!("{prefix}: {}", chunk.join(" ")),
                    text_style(),
                ))
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(pv_lines)
            .style(text_style())
            .block(block("PV 列表"))
            .wrap(Wrap { trim: true }),
        sections[1],
    );
}

fn tab_widget(title: &str, active: bool, focused: bool) -> Paragraph<'static> {
    Paragraph::new(title.to_string())
        .style(if active { tab_active() } else { tab_idle() })
        .block(section_block(title, focused))
}

fn button_widget(title: &str, active: bool, focused: bool) -> Paragraph<'static> {
    Paragraph::new(title.to_string())
        .alignment(Alignment::Center)
        .style(if active { button_on() } else { button_idle() })
        .block(button_block(active, focused))
}

fn block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(border_normal())
        .title(Span::styled(title.to_string(), text_bold()))
}

fn section_block(title: &str, focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(if focused {
            border_focused()
        } else {
            border_normal()
        })
        .title(Span::styled(
            title.to_string(),
            if focused { highlight() } else { text_bold() },
        ))
}

fn button_block(active: bool, focused: bool) -> Block<'static> {
    let border_style = if focused {
        border_focused()
    } else if active {
        border_active()
    } else {
        border_normal()
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

    let mut spans = vec![Span::styled(prompt.to_string(), input_prompt())];
    spans.push(Span::styled(before.to_string(), text_style()));
    match current {
        Some(ch) => {
            spans.push(Span::styled(ch.to_string(), cursor_cell()));
            spans.push(Span::styled(after[ch.len_utf8()..].to_string(), text_style()));
        }
        None => {
            spans.push(Span::styled(" ".to_string(), cursor_cell()));
        }
    }
    Line::from(spans)
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

#[cfg(test)]
mod render_tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::app::App;

    use super::render;

    fn row_text(buf: &ratatui::buffer::Buffer, y: u16, x0: u16, x1: u16) -> String {
        let mut out = String::new();
        for x in x0..=x1 {
            let cell = buf.cell((x, y)).unwrap();
            let ch = cell.symbol();
            if ch.is_empty() {
                out.push('·');
            } else {
                out.push_str(ch);
            }
        }
        out
    }

    #[test]
    fn dump_b_button_rows_in_test_backend() {
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).expect("terminal");
        let app = App::default();
        term.draw(|f| {
            render(f, &app);
        })
        .expect("draw");
        let buf = term.backend().buffer();

        use crate::app::BattleButton;

        fn line_symbols(buf: &ratatui::buffer::Buffer, y: u16) -> String {
            (0..buf.area.width)
                .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                .collect::<Vec<_>>()
                .concat()
        }

        for y in 0..buf.area.height {
            let line = line_symbols(buf, y);
            for button in BattleButton::ALL {
                let label = button.label();
                if line.contains(label) {
                    eprintln!("y={y:02} contains '{label}'");
                }
            }
        }

        let dump: String = (0..buf.area.height)
            .map(|y| format!("{y:02}|{}\n", line_symbols(buf, y)))
            .collect();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("logs/render_dump.txt");
        std::fs::create_dir_all(path.parent().unwrap()).ok();
        std::fs::write(&path, &dump).expect("write dump");
        eprintln!("wrote {}", path.display());

        assert!(
            dump.contains("上一步") || dump.contains("上 一 步"),
            "row3 buttons should be visible in dump"
        );
        assert!(
            dump.lines().any(|line| line.contains("D 实") && {
                let y: u16 = line[..2].trim().parse().unwrap_or(0);
                y >= 15
            }),
            "D panel should start below 12-line button block, got:\n{dump}"
        );
    }

    #[test]
    fn hit_board_cell_a0_matches_layout() {
        use crate::app::Screen;
        use crate::ui::board::cell_hit_point_in_grid;

        let backend = TestBackend::new(120, 40);
        let mut term = Terminal::new(backend).expect("terminal");
        let app = App::default();
        let mut regions = None;
        term.draw(|f| {
            regions = Some(render(f, &app).regions);
        })
        .expect("draw");
        let regions = regions.expect("regions");
        let battle = regions.battle().expect("battle");
        let (col, row) = cell_hit_point_in_grid(battle.board, 0, 9).expect("a0 center");
        let hit = super::hit_test(col, row, Screen::Battle, &regions).expect("hit");
        assert_eq!(hit, super::HitTarget::BoardCell(0, 9));

        let app_rot = {
            let mut a = App::default();
            a.game.rotated = true;
            a
        };
        let mut regions = None;
        term.draw(|f| {
            regions = Some(render(f, &app_rot).regions);
        })
        .expect("draw rotated");
        let regions = regions.expect("regions");
        let battle = regions.battle().expect("battle");
        let (col, row) = cell_hit_point_in_grid(battle.board, 0, 9).expect("a0 center");
        let hit = super::hit_test(col, row, Screen::Battle, &regions).expect("hit");
        assert_eq!(hit, super::HitTarget::BoardCell(8, 0));
    }
}
