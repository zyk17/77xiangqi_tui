mod board;
mod format;
mod help;
mod hit;
mod layout;
pub mod settings_form;
mod style;

use ratatui::{
    Frame,
    layout::Alignment,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
};

use self::style::{
    border_active, border_focused, border_normal, button_idle, button_on, cursor_cell, highlight,
    input_prompt, suggestion, tab_active, tab_idle, text as text_style, text_bold, text_dim,
};

use crate::app::{App, BattleButton, Focus, Screen, SettingsField, TopTab};

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
    SettingsField(SettingsField),
    BoardCell(u8, u8),
}

/// 每个按钮块 3 行高（上下边框 + 文字），共 4 行。
const BUTTON_ROW_HEIGHT: u16 = 3;
const BUTTON_ROW_COUNT: u16 = 4;
const BUTTON_PANEL_HEIGHT: u16 = BUTTON_ROW_HEIGHT * BUTTON_ROW_COUNT;

const BUTTON_ROWS: [[Option<BattleButton>; 3]; 4] = [
    [
        Some(BattleButton::RedAi),
        Some(BattleButton::BlackAi),
        Some(BattleButton::QueryMode),
    ],
    [
        Some(BattleButton::NewGame),
        Some(BattleButton::Undo),
        Some(BattleButton::RotateBoard),
    ],
    [
        Some(BattleButton::PrevMove),
        Some(BattleButton::NextMove),
        Some(BattleButton::CopyFen),
    ],
    [
        Some(BattleButton::PasteFen),
        Some(BattleButton::RealtimeEval),
        None,
    ],
];

pub fn render(frame: &mut Frame<'_>, app: &App) -> RenderOutput {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
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

    if app.help_open {
        help::render_help_overlay(frame, frame.area());
    }

    RenderOutput {
        regions: UiRegions { tabs, screen },
    }
}

pub fn hit_test(column: u16, row: u16, screen: Screen, regions: &UiRegions) -> Option<HitTarget> {
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

fn command_input_height(app: &App) -> u16 {
    let mut h = 3u16;
    if app.screen == Screen::Battle && app.input.slash_menu_open() {
        let n = app.input.suggestions().len() as u16;
        h = h.saturating_add(n.min(14));
    }
    h.max(5)
}

fn render_battle(frame: &mut Frame<'_>, area: Rect, app: &App) -> layout::BattleRegions {
    let cmd_h = command_input_height(app);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(cmd_h)])
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
        board::BoardOverlay {
            last_arrow: app.game.last_move_arrow,
            pending_arrow: app.game.pending_arrow,
            selected: app.game.selected_cell,
            keyboard: board_keyboard_cell(app),
        },
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
        .constraints([
            Constraint::Length(SettingsField::ALL.len() as u16 + 2),
            Constraint::Min(0),
            Constraint::Length(5),
        ])
        .split(area);

    let form_inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(SettingsField::ALL.len() as u16)])
        .split(rows[0]);
    let form_regions = settings_form::render_settings_form(frame, form_inner[0], app);
    frame.render_widget(
        settings_form::form_block(matches!(app.focus, Focus::SettingsField(_))),
        rows[0],
    );

    let hint = settings_form::settings_hint(app.settings_field);
    frame.render_widget(
        Paragraph::new(hint)
            .style(text_style())
            .block(block("说明"))
            .wrap(Wrap { trim: true }),
        rows[1],
    );
    render_command_input(frame, rows[2], app);
    layout::SettingsRegions {
        fields: form_regions.fields,
        field_count: form_regions.field_count,
        command_input: rows[2],
    }
}

fn render_command_input(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let settings_text_edit = app.screen == Screen::Settings && app.focus == Focus::CommandInput;
    let title = if settings_text_edit {
        "C 设置输入*"
    } else if app.focus == Focus::CommandInput {
        "C 输入*"
    } else if app.screen == Screen::Battle && matches!(app.focus, Focus::Board) {
        "C 输入 (:走子 /命令 Tab/→补全)"
    } else {
        "C 输入"
    };
    let input_line = render_input_line(app);
    let mut input_lines = vec![input_line];
    if app.input.slash_menu_open() {
        let suggestions = app.input.suggestions();
        let pick = app.input.slash_pick_index();
        let window = suggestions.len().min(14);
        let start = if suggestions.len() <= window {
            0
        } else {
            pick.saturating_sub(window / 2)
                .min(suggestions.len().saturating_sub(window))
        };
        for (i, command) in suggestions.iter().enumerate().skip(start).take(window) {
            let marker = if i == pick { "▸" } else { " " };
            let style = if i == pick { highlight() } else { suggestion() };
            input_lines.push(Line::from(vec![
                Span::styled(format!("{marker} "), style),
                Span::styled(command.name().to_string(), style),
                Span::styled(format!("  {}", command.description()), text_dim()),
            ]));
        }
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
    let disabled = button.is_disabled(app);
    let active = if disabled {
        false
    } else {
        match button {
            BattleButton::RedAi => app.game.red_ai,
            BattleButton::BlackAi => app.game.black_ai,
            BattleButton::QueryMode => app.game.query_mode,
            BattleButton::RealtimeEval => app.game.realtime_eval,
            _ => false,
        }
    };
    frame.render_widget(
        button_widget(button.label(), active, focused, disabled, button),
        area,
    );
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
            format::format_count_k(app.game.analysis.nps),
            format::format_count_k(app.game.analysis.nodes),
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
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Min(10),
        ],
    )
    .block(block(&format!("D 实时评估 [{}]", app.game.analysis.source)))
    .column_spacing(1)
    .style(text_style())
    .row_highlight_style(highlight());
    frame.render_widget(table, sections[0]);

    let pv = if app.game.history.at_head() {
        &app.game.analysis.pv
    } else {
        app.game.history.pv_at_view()
    };
    let pv_lines = if pv.is_empty() {
        vec![Line::from(Span::styled("PV: --", text_dim()))]
    } else {
        vec![Line::from(Span::styled(
            format!("PV: {}", pv.join(" ")),
            text_style(),
        ))]
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

fn button_widget(
    title: &str,
    active: bool,
    focused: bool,
    disabled: bool,
    button: BattleButton,
) -> Paragraph<'static> {
    Paragraph::new(title.to_string())
        .alignment(Alignment::Center)
        .style(button_text_style(button, active, focused, disabled))
        .block(button_block(active, focused, disabled))
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

fn button_block(active: bool, focused: bool, disabled: bool) -> Block<'static> {
    let border_style = if disabled {
        border_normal()
    } else if focused {
        border_focused()
    } else if active {
        border_active()
    } else {
        border_normal()
    };
    Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
}

fn button_text_style(
    button: BattleButton,
    active: bool,
    focused: bool,
    disabled: bool,
) -> ratatui::style::Style {
    use ratatui::style::Modifier;
    if disabled {
        return style::button_disabled();
    }
    if active {
        return match button {
            BattleButton::RedAi => style::piece_red().add_modifier(Modifier::BOLD),
            BattleButton::BlackAi => style::piece_black().add_modifier(Modifier::BOLD),
            _ => button_on(),
        };
    }
    if focused {
        return highlight();
    }
    button_idle()
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
            spans.push(Span::styled(
                after[ch.len_utf8()..].to_string(),
                text_style(),
            ));
        }
        None => {
            spans.push(Span::styled(" ".to_string(), cursor_cell()));
        }
    }
    Line::from(spans)
}

pub(crate) fn display_or_placeholder(value: &str) -> String {
    if value.is_empty() {
        "<未设置>".to_string()
    } else {
        value.to_string()
    }
}

pub(crate) fn yes_no(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}

fn board_keyboard_cell(app: &App) -> Option<(u8, u8)> {
    if app.screen == Screen::Battle && matches!(app.focus, Focus::Board) {
        Some(app.board_cursor)
    } else {
        None
    }
}

#[cfg(test)]
mod render_tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::App;

    use super::render;

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
            dump.contains("上一步") || dump.contains("◀"),
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
