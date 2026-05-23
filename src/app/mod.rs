use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::{Terminal, backend::Backend, layout::Rect};

use crate::{
    book::BookConfig,
    engine::EngineConfig,
    game::{BoardArrow, GameState},
    input::InputState,
    service::{AppServices, CoordinateMove, ParsedCommand, SlashCommand},
    ui::{self, HitTarget},
};

const TICK_RATE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Battle,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopTab {
    Battle,
    Settings,
}

impl TopTab {
    pub const ALL: [TopTab; 2] = [TopTab::Battle, TopTab::Settings];

    fn next(self) -> Self {
        match self {
            Self::Battle => Self::Settings,
            Self::Settings => Self::Battle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleButton {
    RedAi,
    BlackAi,
    QueryMode,
    RealtimeEval,
    NewGame,
    Undo,
    RotateBoard,
    PrevMove,
    NextMove,
    CopyFen,
    PasteFen,
}

impl BattleButton {
    pub const ALL: [BattleButton; 11] = [
        BattleButton::RedAi,
        BattleButton::BlackAi,
        BattleButton::QueryMode,
        BattleButton::RealtimeEval,
        BattleButton::NewGame,
        BattleButton::Undo,
        BattleButton::RotateBoard,
        BattleButton::PrevMove,
        BattleButton::NextMove,
        BattleButton::CopyFen,
        BattleButton::PasteFen,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::RedAi => "红AI",
            Self::BlackAi => "黑AI",
            Self::QueryMode => "查询模式",
            Self::RealtimeEval => "实时评估",
            Self::NewGame => "新游戏",
            Self::Undo => "悔棋",
            Self::RotateBoard => "旋转棋盘",
            Self::PrevMove => "上一步",
            Self::NextMove => "下一步",
            Self::CopyFen => "复制FEN",
            Self::PasteFen => "粘贴FEN",
        }
    }

    fn step(self, delta: isize) -> Self {
        let index = Self::ALL.iter().position(|item| *item == self).unwrap_or(0) as isize;
        let len = Self::ALL.len() as isize;
        let next = (index + delta).rem_euclid(len) as usize;
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Engine,
    OpeningBook,
}

impl SettingsSection {
    pub const ALL: [SettingsSection; 2] = [SettingsSection::Engine, SettingsSection::OpeningBook];

    pub fn title(self) -> &'static str {
        match self {
            Self::Engine => "引擎设置",
            Self::OpeningBook => "开局库设置",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Engine => Self::OpeningBook,
            Self::OpeningBook => Self::Engine,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    TopTab(TopTab),
    BattleButton(BattleButton),
    CommandInput,
    SettingsSection(SettingsSection),
}

#[derive(Debug, Clone)]
pub struct App {
    pub should_quit: bool,
    pub screen: Screen,
    pub focus: Focus,
    pub game: GameState,
    pub engine: EngineConfig,
    pub book: BookConfig,
    pub status: String,
    pub input: InputState,
    pub board_area: Option<Rect>,
    pub services: AppServices,
}

impl Default for App {
    fn default() -> Self {
        Self {
            should_quit: false,
            screen: Screen::Battle,
            focus: Focus::CommandInput,
            game: GameState::default(),
            engine: EngineConfig::default(),
            book: BookConfig::default(),
            status: "已完成初始骨架。当前重点是棋盘、命令区、按钮区与评估区基建。"
                .to_string(),
            input: InputState::default(),
            board_area: None,
            services: AppServices::default(),
        }
    }
}

impl App {
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| {
                let output = ui::render(frame, self);
                self.board_area = Some(output.board_area);
            })?;

            if !event::poll(TICK_RATE)? {
                continue;
            }

            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    self.on_key(key.code, key.modifiers)
                }
                Event::Mouse(mouse) => self.on_mouse(mouse),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        Ok(())
    }

    fn on_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match code {
            KeyCode::Tab => self.input.autocomplete_next(),
            KeyCode::BackTab => {}
            KeyCode::Left => self.input.move_left(),
            KeyCode::Right => self.input.move_right(),
            KeyCode::Home => self.input.move_home(),
            KeyCode::End => self.input.move_end(),
            KeyCode::Delete => self.input.delete(),
            KeyCode::Backspace => self.command_backspace(),
            KeyCode::Enter => self.submit_command(),
            KeyCode::Char(ch) => self.handle_char(ch),
            _ => {}
        }
    }

    fn on_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }

        if let Some(hit) = ui::hit_test(mouse.column, mouse.row, self.board_area) {
            match hit {
                HitTarget::TopTab(tab) => self.switch_screen(match tab {
                    TopTab::Battle => Screen::Battle,
                    TopTab::Settings => Screen::Settings,
                }),
                HitTarget::BattleButton(button) => {
                    self.activate_battle_button(button);
                    self.focus = Focus::CommandInput;
                }
                HitTarget::CommandInput => {
                    self.focus = Focus::CommandInput;
                }
                HitTarget::SettingsSection(section) => {
                    self.focus = Focus::SettingsSection(section);
                    self.status = format!("已聚焦 {}。后续在这里接表单。", section.title());
                }
                HitTarget::BoardCell(file, rank) => {
                    self.focus = Focus::CommandInput;
                    self.status = format!("预留棋盘点击：file={file}, rank={rank}");
                }
            }
        }
    }

    fn handle_char(&mut self, ch: char) {
        if self.screen != Screen::Battle {
            match ch {
                '1' => self.switch_screen(Screen::Battle),
                '2' => self.switch_screen(Screen::Settings),
                _ => {}
            }
            return;
        }

        if matches!(self.focus, Focus::CommandInput) {
            if ch.is_ascii_alphanumeric() || matches!(ch, '/' | ' ' | '-' | '_') {
                self.input.insert_char(ch);
            }
            return;
        }

        match ch {
            '1' => self.switch_screen(Screen::Battle),
            '2' => self.switch_screen(Screen::Settings),
            _ => {}
        }
    }

    fn command_backspace(&mut self) {
        if matches!(self.focus, Focus::CommandInput) {
            self.input.backspace();
        }
    }

    fn submit_command(&mut self) {
        if !matches!(self.focus, Focus::CommandInput) {
            return;
        }

        let command = self.input.take_text().trim().to_ascii_lowercase();
        if command.is_empty() {
            return;
        }
        match self.services.command.parse(&command) {
            Ok(ParsedCommand::Move(mv)) => self.execute_move_command(mv),
            Ok(ParsedCommand::Slash(command)) => self.execute_slash_command(command),
            Err(err) => self.status = err.message(),
        }
    }

    fn execute_move_command(&mut self, mv: CoordinateMove) {
        self.game.pending_arrow = Some(BoardArrow {
            from_file: mv.from_file,
            from_rank: mv.from_rank,
            to_file: mv.to_file,
            to_rank: mv.to_rank,
        });
        self.status = format!("预留着法输入：{}。后续接入 UCI/UCCI 与棋规校验。", mv.raw);
    }

    fn execute_slash_command(&mut self, command: SlashCommand) {
        match command {
            SlashCommand::New => {
                self.game.reset();
                self.status = format!("已执行 {}，新游戏。", command.name());
            }
            SlashCommand::Undo => self.status = "已识别 /undo。待接历史栈。".to_string(),
            SlashCommand::Prev => self.status = "已识别 /prev。待接浏览模式。".to_string(),
            SlashCommand::Next => self.status = "已识别 /next。待接浏览模式。".to_string(),
            SlashCommand::RedAi => {
                self.game.red_ai = !self.game.red_ai;
                self.status = format!("红AI：{}", if self.game.red_ai { "开启" } else { "关闭" });
            }
            SlashCommand::BlackAi => {
                self.game.black_ai = !self.game.black_ai;
                self.status = format!(
                    "黑AI：{}",
                    if self.game.black_ai {
                        "开启"
                    } else {
                        "关闭"
                    }
                );
            }
            SlashCommand::Query => {
                self.game.query_mode = !self.game.query_mode;
                self.status =
                    format!("查询模式：{}", if self.game.query_mode { "开启" } else { "关闭" });
            }
            SlashCommand::Rotate => {
                self.game.rotated = !self.game.rotated;
                self.status = format!(
                    "棋盘方向：{}",
                    if self.game.rotated {
                        "黑方在下"
                    } else {
                        "红方在下"
                    }
                );
            }
            SlashCommand::Eval => {
                self.game.realtime_eval = !self.game.realtime_eval;
                self.status = format!(
                    "实时评估：{}",
                    if self.game.realtime_eval {
                        "开启"
                    } else {
                        "关闭"
                    }
                );
            }
            SlashCommand::CopyFen => {
                self.status = format!("当前 FEN：{}", self.game.board.to_fen());
            }
            SlashCommand::PasteFen => {
                self.status = "粘贴 FEN 待接系统剪贴板。".to_string();
            }
            SlashCommand::Exit | SlashCommand::Quit => {
                self.should_quit = true;
                self.status = "正在退出。".to_string();
            }
        }
    }

    fn switch_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.focus = match screen {
            Screen::Battle => Focus::CommandInput,
            Screen::Settings => Focus::SettingsSection(SettingsSection::Engine),
        };
    }

    fn activate_battle_button(&mut self, button: BattleButton) {
        match button {
            BattleButton::RedAi => {
                self.game.red_ai = !self.game.red_ai;
                self.status = format!("红AI：{}", if self.game.red_ai { "开启" } else { "关闭" });
            }
            BattleButton::BlackAi => {
                self.game.black_ai = !self.game.black_ai;
                self.status = format!(
                    "黑AI：{}",
                    if self.game.black_ai {
                        "开启"
                    } else {
                        "关闭"
                    }
                );
            }
            BattleButton::QueryMode => {
                self.game.query_mode = !self.game.query_mode;
                self.status = format!(
                    "查询模式：{}",
                    if self.game.query_mode {
                        "开启"
                    } else {
                        "关闭"
                    }
                );
            }
            BattleButton::RealtimeEval => {
                self.game.realtime_eval = !self.game.realtime_eval;
                self.status = format!(
                    "实时评估：{}",
                    if self.game.realtime_eval {
                        "开启"
                    } else {
                        "关闭"
                    }
                );
            }
            BattleButton::NewGame => {
                self.game.reset();
                self.status = "已重置到初始局面。".to_string();
            }
            BattleButton::Undo => self.status = "悔棋逻辑待接入。".to_string(),
            BattleButton::RotateBoard => {
                self.game.rotated = !self.game.rotated;
                self.status = format!(
                    "棋盘方向：{}",
                    if self.game.rotated {
                        "黑方在下"
                    } else {
                        "红方在下"
                    }
                );
            }
            BattleButton::PrevMove => self.status = "上一步浏览逻辑待接入。".to_string(),
            BattleButton::NextMove => self.status = "下一步浏览逻辑待接入。".to_string(),
            BattleButton::CopyFen => {
                self.status = format!("当前 FEN：{}", self.game.board.to_fen())
            }
            BattleButton::PasteFen => self.status = "粘贴 FEN 待接系统剪贴板。".to_string(),
        }
    }
}
