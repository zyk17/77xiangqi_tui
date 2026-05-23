use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::{Terminal, backend::Backend, layout::Rect};

use crate::{
    book::BookConfig,
    engine::EngineConfig,
    game::GameState,
    input::InputState,
    service::{AppServices, CoordinateMove, GameService, ParsedCommand, SlashCommand},
    xiangqi::Side,
    settings_config,
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

#[derive(Debug)]
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
    last_analysis_revision: u64,
}

impl Default for App {
    fn default() -> Self {
        let mut engine = EngineConfig::default();
        engine.path = settings_config::load_engine_path();
        let status = if engine.path.is_empty() {
            "就绪。在「设置」中填写引擎路径，或设置环境变量 XIANGQI_ENGINE_PATH。".to_string()
        } else {
            format!("已加载引擎路径：{}", engine.path)
        };
        Self {
            should_quit: false,
            screen: Screen::Battle,
            focus: Focus::CommandInput,
            game: GameState::default(),
            engine,
            book: BookConfig::default(),
            status,
            input: InputState::default(),
            board_area: None,
            services: AppServices::default(),
            last_analysis_revision: 0,
        }
    }
}

impl App {
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()> {
        while !self.should_quit {
            self.tick_engine_stream();
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

        self.services.engine.stop_stream();
        Ok(())
    }

    /// 流式引擎：后台 `go infinite` 写共享快照，每帧刷新 `D` 区（对齐 GUI `analysis_stream` 思路，无 JSON/Tauri）。
    fn tick_engine_stream(&mut self) {
        if self.screen != Screen::Battle {
            self.services.engine.stop_stream();
            return;
        }
        let fen = GameService::engine_fen(&self.game);
        let want_stream = self.game.realtime_eval || self.game.query_mode;
        self.services
            .engine
            .ensure_stream(&fen, &self.engine, want_stream);
        if !want_stream {
            return;
        }
        let Some((store, revision)) = self
            .services
            .engine
            .snapshot_if_newer(self.last_analysis_revision)
        else {
            return;
        };
        self.last_analysis_revision = revision;
        self.services.analysis.apply_engine_store(
            &mut self.game.analysis,
            &store,
            self.game.query_mode,
            &mut self.game.pending_arrow,
        );
    }

    fn refresh_engine_after_mode_change(&mut self) {
        self.tick_engine_stream();
        if self.game.realtime_eval || self.game.query_mode {
            self.status = format!(
                "引擎流式分析中{}",
                if self.services.engine.is_streaming() {
                    ""
                } else if self.engine.path.trim().is_empty() {
                    "（请先在设置中配置引擎路径）"
                } else {
                    "（等待引擎输出）"
                }
            );
        } else {
            self.services.engine.stop_stream();
            self.game.pending_arrow = None;
            self.game.analysis = self.services.analysis.idle_snapshot();
            self.last_analysis_revision = 0;
            self.status = "已关闭实时评估/查询。".to_string();
        }
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

        if let Some(hit) = ui::hit_test(
            mouse.column,
            mouse.row,
            self.board_area,
            self.game.rotated,
            self.game.last_move_uci.is_some(),
        ) {
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
                    if section == SettingsSection::Engine {
                        self.input.set_text(&self.engine.path);
                        self.status =
                            "编辑引擎路径后按 Enter 保存（写入 xiangqi_tui.conf）。".to_string();
                    } else {
                        self.status = format!("已聚焦 {}。", section.title());
                    }
                }
                HitTarget::BoardCell(file, rank) => {
                    self.focus = Focus::CommandInput;
                    if let Some(uci) = GameService::try_click_cell(&mut self.game, file, rank)
                    {
                        self.apply_uci_move(&uci);
                    } else if self.game.selected_cell == Some((file, rank)) {
                        self.status = format!(
                            "已选 {}{}，请点目标格。",
                            (b'a' + file) as char,
                            9 - rank
                        );
                    } else if !self.game.history.at_head() {
                        self.status = "浏览历史中，请 /next 回到最新步再走子。".to_string();
                    } else {
                        self.status = "请先选择己方棋子。".to_string();
                    }
                }
            }
        }
    }

    fn handle_char(&mut self, ch: char) {
        if self.screen == Screen::Settings {
            match ch {
                '1' => self.switch_screen(Screen::Battle),
                '2' => self.switch_screen(Screen::Settings),
                _ => {}
            }
            if matches!(self.focus, Focus::SettingsSection(SettingsSection::Engine)) {
                if ch.is_ascii() || matches!(ch, ' ' | '\\' | '/' | ':' | '.' | '-' | '_') {
                    self.input.insert_char(ch);
                }
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
        if self.screen == Screen::Settings
            && matches!(self.focus, Focus::SettingsSection(SettingsSection::Engine))
        {
            self.input.backspace();
            return;
        }
        if matches!(self.focus, Focus::CommandInput) {
            self.input.backspace();
        }
    }

    fn submit_command(&mut self) {
        if self.screen == Screen::Settings
            && matches!(self.focus, Focus::SettingsSection(SettingsSection::Engine))
        {
            self.submit_settings_engine_path();
            return;
        }
        if !matches!(self.focus, Focus::CommandInput) {
            return;
        }

        let raw = self.input.take_text();
        let command = raw.trim().to_ascii_lowercase();
        if command.is_empty() {
            return;
        }
        match self.services.command.parse(&command) {
            Ok(ParsedCommand::Move(mv)) => self.execute_move_command(mv),
            Ok(ParsedCommand::Slash(slash)) => {
                self.execute_slash_command(slash, raw.trim());
            }
            Err(err) => self.status = err.message(),
        }
    }

    fn execute_move_command(&mut self, mv: CoordinateMove) {
        self.apply_uci_move(&mv.raw);
    }

    fn apply_uci_move(&mut self, uci: &str) {
        match GameService::apply_uci(&mut self.game, uci) {
            Ok(()) => {
                self.last_analysis_revision = 0;
                self.refresh_engine_after_position_change();
                let side = match self.game.side_to_move {
                    Side::Red => "红",
                    Side::Black => "黑",
                };
                self.status = format!("已走 {uci}，轮到{side}方。");
            }
            Err(err) => self.status = err.message(),
        }
    }

    fn refresh_engine_after_position_change(&mut self) {
        if self.game.realtime_eval || self.game.query_mode {
            self.services.engine.stop_stream();
            self.last_analysis_revision = 0;
        }
        self.tick_engine_stream();
    }

    fn execute_slash_command(&mut self, command: SlashCommand, raw: &str) {
        match command {
            SlashCommand::New => {
                GameService::reset(&mut self.game);
                self.refresh_engine_after_mode_change();
                self.status = format!("已执行 {}，新游戏。", command.name());
            }
            SlashCommand::Undo => {
                if GameService::undo(&mut self.game) {
                    self.refresh_engine_after_position_change();
                    self.status = "已悔棋。".to_string();
                } else {
                    self.status = "无法悔棋（已在初始局面）。".to_string();
                }
            }
            SlashCommand::Prev => {
                if GameService::go_prev(&mut self.game) {
                    self.refresh_engine_after_position_change();
                    self.status = match self.game.last_move_uci.as_deref() {
                        Some(m) => format!("浏览历史；当前局面上一手 {m}。"),
                        None => "浏览历史；当前为初始局面。".to_string(),
                    };
                } else {
                    self.status = "已在第一步。".to_string();
                }
            }
            SlashCommand::Next => {
                if GameService::go_next(&mut self.game) {
                    self.refresh_engine_after_position_change();
                    self.status = "浏览下一步。".to_string();
                } else {
                    self.status = "已在最新步。".to_string();
                }
            }
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
                self.refresh_engine_after_mode_change();
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
                self.refresh_engine_after_mode_change();
            }
            SlashCommand::CopyFen => {
                self.status = format!("当前 FEN：{}", GameService::engine_fen(&self.game));
            }
            SlashCommand::PasteFen => {
                let fen = raw
                    .strip_prefix("/pastefen")
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                match fen {
                    Some(fen) => match GameService::load_fen(&mut self.game, fen) {
                        Ok(()) => {
                            self.refresh_engine_after_position_change();
                            self.status = "已载入 FEN。".to_string();
                        }
                        Err(msg) => self.status = msg,
                    },
                    None => self.status = "用法：/pastefen <FEN>".to_string(),
                }
            }
            SlashCommand::Exit | SlashCommand::Quit => {
                self.should_quit = true;
                self.status = "正在退出。".to_string();
            }
        }
    }

    fn submit_settings_engine_path(&mut self) {
        let path = self.input.take_text();
        let path = path.trim().to_string();
        if path.is_empty() {
            self.status = "引擎路径为空，未保存。".to_string();
            return;
        }
        self.engine.path = path;
        if let Err(err) = settings_config::save_engine_path(&self.engine.path) {
            self.status = format!("保存配置失败：{err}");
            return;
        }
        self.services.engine.stop_stream();
        self.last_analysis_revision = 0;
        self.refresh_engine_after_mode_change();
        self.status = format!("已保存引擎路径：{}", self.engine.path);
    }

    fn switch_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.input.clear();
        self.focus = match screen {
            Screen::Battle => Focus::CommandInput,
            Screen::Settings => Focus::SettingsSection(SettingsSection::Engine),
        };
        if screen == Screen::Settings {
            self.input.set_text(&self.engine.path);
            self.status =
                "在下方输入框编辑引擎路径，Enter 保存。也可使用环境变量 XIANGQI_ENGINE_PATH。".to_string();
        }
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
                self.refresh_engine_after_mode_change();
            }
            BattleButton::RealtimeEval => {
                self.game.realtime_eval = !self.game.realtime_eval;
                self.refresh_engine_after_mode_change();
            }
            BattleButton::NewGame => {
                GameService::reset(&mut self.game);
                self.refresh_engine_after_mode_change();
                self.status = "已重置到初始局面。".to_string();
            }
            BattleButton::Undo => {
                if GameService::undo(&mut self.game) {
                    self.refresh_engine_after_position_change();
                    self.status = "已悔棋。".to_string();
                } else {
                    self.status = "无法悔棋。".to_string();
                }
            }
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
            BattleButton::PrevMove => {
                if GameService::go_prev(&mut self.game) {
                    self.refresh_engine_after_position_change();
                    self.status = "浏览上一步。".to_string();
                } else {
                    self.status = "已在第一步。".to_string();
                }
            }
            BattleButton::NextMove => {
                if GameService::go_next(&mut self.game) {
                    self.refresh_engine_after_position_change();
                    self.status = "浏览下一步。".to_string();
                } else {
                    self.status = "已在最新步。".to_string();
                }
            }
            BattleButton::CopyFen => {
                self.status = format!("当前 FEN：{}", GameService::engine_fen(&self.game))
            }
            BattleButton::PasteFen => {
                self.status = "在 C 区输入：/pastefen <FEN>".to_string();
            }
        }
    }
}
