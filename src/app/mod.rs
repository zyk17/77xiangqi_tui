pub mod settings_field;

use std::time::{Duration, Instant};

use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::{Terminal, backend::Backend};

use crate::{
    book::BookConfig,
    clipboard,
    engine::EngineConfig,
    game::GameState,
    input::InputState,
    service::{
        AI_MOVE_DELAY, AiPhase, AppServices, AutoplayService, BOOK_ARROW_DELAY, CoordinateMove,
        GameService, ParsedCommand, SlashCommand, ai_enabled_for_side, best_uci_from_engine,
        book_config_usable, should_query_book_for_display,
    },
    settings_config,
    ui::{self, HitTarget},
    xiangqi::{Side, uci_cell_label},
};

pub use settings_field::SettingsField;
use settings_field::{
    SettingsFieldKind, bump_hash_mb, clamp_threads, cycle_pick_mode, cycle_protocol,
};

const TICK_RATE: Duration = Duration::from_millis(50);
const AI_ENGINE_RETRY_COOLDOWN: Duration = Duration::from_secs(2);

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
    #[cfg(test)]
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

    /// 与 GUI `i18n` 对弈控制条文案对齐（略缩短以适配三列按钮格）。
    pub fn label(self) -> &'static str {
        match self {
            Self::RedAi => "🔴红电脑",
            Self::BlackAi => "⚫黑电脑",
            Self::QueryMode => "🤖查询",
            Self::RealtimeEval => "📊实时",
            Self::NewGame => "✦新局",
            Self::Undo => "↩悔棋",
            Self::RotateBoard => "↻旋转",
            Self::PrevMove => "◀上一步",
            Self::NextMove => "下一步▶",
            Self::CopyFen => "📋FEN",
            Self::PasteFen => "📥粘贴",
        }
    }

    pub fn is_disabled(self, app: &App) -> bool {
        match self {
            Self::RedAi => {
                app.game.query_mode || (app.game.is_game_over() && !app.game.red_ai)
            }
            Self::BlackAi => {
                app.game.query_mode || (app.game.is_game_over() && !app.game.black_ai)
            }
            Self::QueryMode => {
                app.game.red_ai
                    || app.game.black_ai
                    || (app.game.is_game_over() && !app.game.query_mode)
            }
            Self::RealtimeEval => app.game.is_game_over() && !app.game.realtime_eval,
            Self::Undo => !app.game.history.can_undo(),
            Self::PrevMove => !app.game.history.can_go_prev(),
            Self::NextMove => !app.game.history.can_go_next(),
            _ => false,
        }
    }

    pub fn disabled_reason(self, app: &App) -> Option<&'static str> {
        if !self.is_disabled(app) {
            return None;
        }
        Some(match self {
            Self::RedAi | Self::BlackAi if app.game.is_game_over() => "对局已结束，请点「新局」或 /new。",
            Self::RedAi | Self::BlackAi => "请先关闭查询模式。",
            Self::QueryMode if app.game.is_game_over() => "对局已结束，请点「新局」或 /new。",
            Self::QueryMode => "请先关闭红/黑电脑。",
            Self::RealtimeEval if app.game.is_game_over() => "对局已结束，请点「新局」或 /new。",
            Self::Undo => "无法悔棋。",
            Self::PrevMove => "已在第一步。",
            Self::NextMove => "已在最新步。",
            _ => "当前不可用。",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    TopTab(TopTab),
    BattleButton(BattleButton),
    Board,
    CommandInput,
    SettingsField(SettingsField),
}

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub screen: Screen,
    pub focus: Focus,
    pub game: GameState,
    pub engine: EngineConfig,
    pub book: BookConfig,
    pub settings_field: SettingsField,
    pub status: String,
    pub input: InputState,
    pub ui_regions: Option<ui::UiRegions>,
    pub services: AppServices,
    last_analysis_revision: u64,
    last_book_fen: String,
    book_blocks_engine: bool,
    ai_phase: AiPhase,
    /// 引擎分析失败后的冷却，避免每帧重试卡死 UI。
    ai_engine_retry_after: Option<Instant>,
    /// 键盘焦点格（内部 file/rank，与 UCI 一致）
    pub board_cursor: (u8, u8),
    /// `/help` 或 `?` 打开的操作说明浮层。
    pub help_open: bool,
}

impl Default for App {
    fn default() -> Self {
        let engine = EngineConfig {
            path: settings_config::load_engine_path(),
            protocol: settings_config::load_engine_protocol(),
            threads: settings_config::load_engine_threads(),
            hash_mb: settings_config::load_engine_hash_mb(),
            skill_level: settings_config::load_engine_skill(),
            multi_pv: settings_config::load_engine_multi_pv(),
            ..EngineConfig::default()
        };
        let book = BookConfig {
            local_path: settings_config::load_book_local_path(),
            local_enabled: settings_config::load_book_local_enabled(),
            cloud_enabled: settings_config::load_book_cloud_enabled(),
            pick_mode: settings_config::load_book_pick_mode(),
            max_halfmoves: settings_config::load_book_max_halfmoves(),
        };
        let status = if engine.path.is_empty() {
            "就绪。在「设置」中填写引擎路径，或设置环境变量 XIANGQI_ENGINE_PATH。".to_string()
        } else {
            format!("已加载引擎路径：{}", engine.path)
        };
        Self {
            should_quit: false,
            screen: Screen::Battle,
            focus: Focus::Board,
            board_cursor: (7, 7),
            game: GameState::default(),
            engine,
            book,
            settings_field: SettingsField::EnginePath,
            status,
            input: InputState::default(),
            ui_regions: None,
            services: AppServices::default(),
            last_analysis_revision: 0,
            last_book_fen: String::new(),
            book_blocks_engine: false,
            ai_phase: AiPhase::Idle,
            ai_engine_retry_after: None,
            help_open: false,
        }
    }
}

impl App {
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()> {
        while !self.should_quit {
            // AI 先走子再开流式，避免 infinite 与 autoplay 抢同一引擎锁/进程
            if ai_enabled_for_side(&self.game) && self.screen == Screen::Battle {
                self.tick_ai_autoplay();
                self.tick_engine_stream();
            } else {
                self.tick_engine_stream();
                self.tick_ai_autoplay();
            }
            terminal.draw(|frame| {
                let output = ui::render(frame, self);
                self.ui_regions = Some(output.regions);
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

    fn want_engine_stream(&self) -> bool {
        !self.game.is_game_over()
            && (self.game.realtime_eval || self.game.query_mode)
            && !self.book_blocks_engine
            && !ai_enabled_for_side(&self.game)
            && matches!(self.ai_phase, AiPhase::Idle)
    }

    fn refresh_book_analysis(&mut self, fen: &str) {
        if !should_query_book_for_display(&self.book) {
            self.book_blocks_engine = false;
            return;
        }
        if self.last_book_fen == fen {
            return;
        }
        self.last_book_fen = fen.to_string();
        let query_mode = self.game.query_mode;
        let hit = AutoplayService::query_book_display(
            &self.services.book,
            &self.services.analysis,
            &mut self.game,
            &self.book,
            fen,
            query_mode,
        );
        self.book_blocks_engine = hit && (self.game.query_mode || self.game.realtime_eval);
        if hit && !self.book_blocks_engine {
            self.last_analysis_revision = 0;
        }
    }

    /// 流式引擎：后台 `go infinite` 写共享快照，每帧刷新 `D` 区（对齐 GUI `analysis_stream` 思路，无 JSON/Tauri）。
    fn tick_engine_stream(&mut self) {
        if self.game.is_game_over() {
            self.services.engine.stop_stream();
            return;
        }
        if self.screen != Screen::Battle {
            self.services.engine.stop_stream();
            return;
        }
        if !self.game.history.at_head() {
            self.services.engine.stop_stream();
            return;
        }
        let fen = GameService::engine_fen(&self.game);
        let want_eval = self.game.realtime_eval || self.game.query_mode;
        if want_eval || book_config_usable(&self.book) {
            self.refresh_book_analysis(&fen);
        }
        let want_stream = self.want_engine_stream();
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

    fn reset_analysis_tracking(&mut self) {
        self.last_analysis_revision = 0;
        self.last_book_fen.clear();
        self.book_blocks_engine = false;
        self.ai_phase = AiPhase::Idle;
        self.ai_engine_retry_after = None;
    }

    fn tick_ai_autoplay(&mut self) {
        if self.game.is_game_over() {
            self.ai_phase = AiPhase::Idle;
            return;
        }
        if self.screen != Screen::Battle {
            self.ai_phase = AiPhase::Idle;
            return;
        }

        if let AiPhase::WaitingToApply { uci, ready_at } = &self.ai_phase {
            if Instant::now() < *ready_at {
                return;
            }
            let uci = uci.clone();
            self.ai_phase = AiPhase::Idle;
            self.game.pending_arrow = None;
            match GameService::apply_uci(&mut self.game, &uci) {
                Ok(()) => {
                    self.reset_analysis_tracking();
                    self.refresh_engine_after_position_change();
                    let side = match self.game.side_to_move {
                        Side::Red => "红",
                        Side::Black => "黑",
                    };
                    self.status = format!("AI 已走 {uci}，轮到{side}方。");
                }
                Err(err) => self.status = err.message(),
            }
            return;
        }

        if !ai_enabled_for_side(&self.game) {
            self.ai_phase = AiPhase::Idle;
            return;
        }
        if self.game.query_mode {
            return;
        }
        if !self.game.history.at_head() {
            return;
        }

        let fen = GameService::engine_fen(&self.game);
        if let Some(uci) = AutoplayService::try_book_autoplay_move_for_game(
            &self.services.book,
            &self.game,
            &self.book,
            &fen,
        ) {
            self.ai_phase = AutoplayService::begin_ai_wait(
                &mut self.game,
                uci,
                BOOK_ARROW_DELAY.max(AI_MOVE_DELAY),
            );
            return;
        }

        if self.engine.path.trim().is_empty() {
            self.status = "红/黑电脑：请先在设置中配置引擎路径。".to_string();
            return;
        }

        if self
            .ai_engine_retry_after
            .is_some_and(|t| Instant::now() < t)
        {
            return;
        }

        self.status = "电脑思考中（引擎分析）…".to_string();
        let result = self.services.engine.run_autoplay_once(&fen, &self.engine);
        if let Some(uci) = best_uci_from_engine(&result) {
            self.ai_engine_retry_after = None;
            self.services
                .analysis
                .apply_engine_result(&mut self.game.analysis, &result, &fen);
            self.ai_phase = AutoplayService::begin_ai_wait(&mut self.game, uci, AI_MOVE_DELAY);
        } else {
            self.ai_engine_retry_after = Some(Instant::now() + AI_ENGINE_RETRY_COOLDOWN);
            self.status = format!(
                "引擎未返回合法着法（best={}，{}s 后重试；XIANGQI_TUI_DEBUG=1 见 logs/runtime.log）。",
                result.best_move,
                AI_ENGINE_RETRY_COOLDOWN.as_secs()
            );
        }
    }

    fn refresh_view_after_rotate(&mut self) {
        GameService::sync_view_after_rotate(&mut self.game);
        if self.game.analysis.source != "engine" {
            return;
        }
        if !(self.game.realtime_eval || self.game.query_mode) {
            return;
        }
        let fen = GameService::engine_fen(&self.game);
        let store = self.services.engine.current_store();
        if store.fen != fen {
            return;
        }
        let best = store.result.best_move.trim();
        if best.is_empty() || best == "stub_move" {
            return;
        }
        self.services.analysis.apply_engine_result(
            &mut self.game.analysis,
            &store.result,
            &store.fen,
        );
        self.services.analysis.sync_query_arrow(
            best,
            self.game.query_mode,
            &mut self.game.pending_arrow,
        );
    }

    /// 停止引擎流、自动走子及所有分析/查询模式（对齐 GUI 新局/停分析）。
    fn stop_all_activity(&mut self) {
        self.game.red_ai = false;
        self.game.black_ai = false;
        self.game.query_mode = false;
        self.game.realtime_eval = false;
        self.ai_phase = AiPhase::Idle;
        self.ai_engine_retry_after = None;
        self.game.pending_arrow = None;
        self.services.engine.stop_stream();
        self.game.analysis = self.services.analysis.idle_snapshot();
        self.reset_analysis_tracking();
    }

    fn start_new_game(&mut self) {
        self.stop_all_activity();
        GameService::reset(&mut self.game);
        GameService::refresh_game_over(&mut self.game);
        self.board_cursor = (7, 7);
        self.focus = Focus::Board;
        self.status = "新游戏：已停止全部模式，回到初始局面。".to_string();
    }

    /// 最新步变为终局时：只走停止逻辑，不重置棋谱、不自动新局。
    fn on_position_changed(&mut self) {
        if !self.game.history.at_head() {
            return;
        }
        let was_over = self.game.is_game_over();
        GameService::refresh_game_over(&mut self.game);
        let now_over = self.game.is_game_over();
        if now_over && !was_over {
            let msg = self
                .game
                .game_over
                .clone()
                .unwrap_or_else(|| "对局结束".to_string());
            self.stop_all_activity();
            self.game.selected_cell = None;
            self.status = format!(
                "对局结束：{msg}。已停止模式与引擎流；可用上一步/下一步浏览棋谱，/new 开新局。"
            );
        } else if !now_over && was_over {
            self.status =
                "已离开终局，可继续对弈（分析/电脑模式需手动重新开启）。".to_string();
        }
    }

    fn history_step_status(&self, prev: bool) -> String {
        let detail = if prev {
            match self.game.last_move_uci.as_deref() {
                Some(m) => format!("浏览上一步（上一手 {m}）"),
                None => "浏览上一步（初始局面）".to_string(),
            }
        } else if self.game.history.at_head() && self.game.is_game_over() {
            "浏览至最新步（终局局面）".to_string()
        } else {
            "浏览下一步".to_string()
        };
        self.status_with_session_over(detail)
    }

    fn status_with_session_over(&self, detail: impl Into<String>) -> String {
        let detail = detail.into();
        if let Some(msg) = &self.game.game_over {
            if !self.game.history.at_head() {
                return format!("{detail}（本盘已结束：{msg}）");
            }
        }
        detail
    }

    fn copy_fen_to_clipboard(&mut self) {
        let fen = GameService::engine_fen(&self.game);
        match clipboard::copy_text(&fen) {
            Ok(()) => self.status = format!("已复制 FEN 到剪贴板：{fen}"),
            Err(err) => self.status = err,
        }
    }

    fn refresh_engine_after_mode_change(&mut self) {
        self.tick_engine_stream();
        if self.game.is_game_over() {
            if self.game.realtime_eval || self.game.query_mode {
                self.status = "对局已结束，实时评估/查询已停用；请 /new 开新局。".to_string();
            } else {
                self.services.engine.stop_stream();
                self.game.pending_arrow = None;
                self.game.analysis = self.services.analysis.idle_snapshot();
                self.reset_analysis_tracking();
                self.status = "已关闭实时评估/查询。".to_string();
            }
            return;
        }
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
            self.reset_analysis_tracking();
            if book_config_usable(&self.book) {
                self.tick_engine_stream();
            }
            self.status = "已关闭实时评估/查询。".to_string();
        }
    }

    fn on_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        if self.help_open {
            if code == KeyCode::Esc {
                self.help_open = false;
                self.status = "已关闭帮助。".to_string();
            }
            return;
        }

        if code == KeyCode::Tab {
            if matches!(self.focus, Focus::CommandInput) {
                self.command_tab_complete();
            } else {
                self.toggle_screen_tab();
            }
            return;
        }

        if self.screen == Screen::Settings {
            self.on_key_settings(code);
            return;
        }

        self.on_key_battle(code);
    }

    fn toggle_screen_tab(&mut self) {
        let next = match self.screen {
            Screen::Battle => Screen::Settings,
            Screen::Settings => Screen::Battle,
        };
        self.switch_screen(next);
        self.status = match next {
            Screen::Battle => {
                "对弈：Tab 切设置；棋盘方向键+空格；: / 命令（Tab/→ 补全）。".to_string()
            }
            Screen::Settings => "设置：Tab 切对弈；↑↓ 选行；Enter 在 C 区编辑。".to_string(),
        };
    }

    /// C 区命令输入时 Tab 补全（与 → 相同）；非输入焦点时由 `toggle_screen_tab` 切页。
    fn command_tab_complete(&mut self) {
        if self.screen == Screen::Battle && self.input.try_slash_complete() {
            self.status = "Tab 已补全命令，Enter 执行，↑↓ 换选。".to_string();
        }
    }

    fn on_key_battle(&mut self, code: KeyCode) {
        if matches!(self.focus, Focus::CommandInput)
            && self.handle_command_input_key(code, "棋盘：方向键移动，空格选子/落子，: 命令，/ 命令列表。")
        {
            return;
        }

        match code {
            KeyCode::Esc => {}
            KeyCode::Char('?') => {
                self.help_open = true;
                self.status = "操作说明（Esc 关闭）。".to_string();
            }
            KeyCode::Char(':') => {
                self.focus = Focus::CommandInput;
                self.input.clear();
                self.status = "命令模式（Enter 执行，Esc 返回棋盘）。".to_string();
            }
            KeyCode::Char('/') => {
                self.focus = Focus::CommandInput;
                self.input.set_text("/");
                self.status = "命令列表：↑↓ 选择，Tab/→ 补全，Enter 执行，Esc 返回。".to_string();
            }
            KeyCode::Up => self.move_board_cursor(0, -1),
            KeyCode::Down => self.move_board_cursor(0, 1),
            KeyCode::Left => self.move_board_cursor(-1, 0),
            KeyCode::Right => self.move_board_cursor(1, 0),
            KeyCode::Char(' ') => self.board_space(),
            KeyCode::Char(ch) => self.handle_char(ch),
            _ => {}
        }
    }

    fn move_board_cursor(&mut self, dfile: i8, drank: i8) {
        self.focus = Focus::Board;
        let (file, rank) = self.board_cursor;
        let file = (i16::from(file) + i16::from(dfile)).clamp(0, 8) as u8;
        let rank = (i16::from(rank) + i16::from(drank)).clamp(0, 9) as u8;
        self.board_cursor = (file, rank);
        self.status = format!("光标 {}。", uci_cell_label(file, rank));
    }

    fn board_space(&mut self) {
        self.focus = Focus::Board;
        let (file, rank) = self.board_cursor;
        self.on_board_cell(file, rank);
    }

    fn on_board_cell(&mut self, file: u8, rank: u8) {
        let prev = self.game.selected_cell;
        if let Some(uci) = GameService::try_click_cell(&mut self.game, file, rank) {
            self.apply_uci_move(&uci);
            return;
        }
        if !self.game.history.at_head() {
            self.status = "浏览历史中，请 /next 回到最新步再走子。".to_string();
            return;
        }
        let Some(sel) = self.game.selected_cell else {
            self.status = "请先选择己方棋子。".to_string();
            return;
        };
        if prev.is_some() && prev != Some(sel) {
            self.status = format!("已改选 {}。", uci_cell_label(sel.0, sel.1));
        } else {
            self.status = format!(
                "已选 {}，请点目标格或点其他己方棋子改选。",
                uci_cell_label(sel.0, sel.1)
            );
        }
    }

    fn handle_command_input_key(&mut self, code: KeyCode, esc_status: &str) -> bool {
        match code {
            KeyCode::Esc => {
                if self.screen == Screen::Settings {
                    self.focus = Focus::SettingsField(self.settings_field);
                    self.status = ui::settings_form::settings_hint(self.settings_field);
                } else {
                    self.focus = Focus::Board;
                    if self.input.slash_menu_open() {
                        self.input.clear();
                    }
                    self.status = esc_status.to_string();
                }
                true
            }
            KeyCode::Up if self.input.history_prev() => {
                self.status = "上一条命令（↑↓ 翻阅，Enter 才记入历史）。".to_string();
                true
            }
            KeyCode::Up if self.input.slash_menu_open() => {
                self.input.move_slash_pick(-1);
                true
            }
            KeyCode::Down if self.input.history_next() => {
                self.status = "下一条命令。".to_string();
                true
            }
            KeyCode::Down if self.input.slash_menu_open() => {
                self.input.move_slash_pick(1);
                true
            }
            KeyCode::Right if self.input.slash_menu_open() => {
                self.input.apply_slash_pick_to_buffer();
                self.status = "已补全命令（Tab/→），Enter 执行，↑↓ 换选。".to_string();
                true
            }
            KeyCode::Enter if self.input.slash_menu_open() => {
                self.input.apply_slash_pick_to_buffer();
                if self.screen == Screen::Settings {
                    self.submit_settings_text_field();
                } else {
                    self.submit_command();
                }
                true
            }
            KeyCode::Enter => {
                if self.screen == Screen::Settings {
                    self.submit_settings_text_field();
                } else {
                    self.submit_command();
                }
                true
            }
            KeyCode::Left => {
                self.input.move_left();
                true
            }
            KeyCode::Right => {
                self.input.move_right();
                true
            }
            KeyCode::Home => {
                self.input.move_home();
                true
            }
            KeyCode::End => {
                self.input.move_end();
                true
            }
            KeyCode::Delete => {
                self.input.delete();
                true
            }
            KeyCode::Backspace => {
                self.command_backspace();
                true
            }
            KeyCode::Char(ch) => {
                if self.screen == Screen::Settings {
                    self.settings_input_char(ch);
                } else {
                    self.handle_char(ch);
                }
                true
            }
            _ => false,
        }
    }

    fn on_key_settings(&mut self, code: KeyCode) {
        if matches!(self.focus, Focus::CommandInput)
            && self.handle_command_input_key(
                code,
                &format!("编辑「{}」：Enter 保存，Esc 返回。", self.settings_field.label()),
            )
        {
            return;
        }

        match code {
            KeyCode::Up => {
                self.settings_field = self.settings_field.prev();
                self.focus_settings_field(self.settings_field);
            }
            KeyCode::Down => {
                self.settings_field = self.settings_field.next();
                self.focus_settings_field(self.settings_field);
            }
            KeyCode::Left => self.settings_adjust_field(-1),
            KeyCode::Right => self.settings_adjust_field(1),
            KeyCode::Char(' ') => self.settings_toggle_field(),
            KeyCode::Enter => self.settings_enter_field(),
            _ => {}
        }
    }

    fn on_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }

        let Some(regions) = self.ui_regions else {
            return;
        };
        if let Some(hit) = ui::hit_test(mouse.column, mouse.row, self.screen, &regions) {
            match hit {
                HitTarget::TopTab(tab) => self.switch_screen(match tab {
                    TopTab::Battle => Screen::Battle,
                    TopTab::Settings => Screen::Settings,
                }),
                HitTarget::BattleButton(button) => {
                    if button.is_disabled(self) {
                        if let Some(reason) = button.disabled_reason(self) {
                            self.status = reason.to_string();
                        }
                    } else {
                        self.activate_battle_button(button);
                    }
                    self.focus = Focus::Board;
                }
                HitTarget::CommandInput => {
                    if self.screen == Screen::Settings {
                        self.begin_settings_input();
                    } else {
                        self.focus = Focus::CommandInput;
                    }
                }
                HitTarget::SettingsField(field) => {
                    self.settings_field = field;
                    self.focus_settings_field(field);
                }
                HitTarget::BoardCell(file, rank) => {
                    self.board_cursor = (file, rank);
                    self.focus = Focus::Board;
                    self.on_board_cell(file, rank);
                }
            }
        }
    }

    fn handle_char(&mut self, ch: char) {
        if matches!(self.focus, Focus::CommandInput) {
            if self.screen == Screen::Settings {
                self.settings_input_char(ch);
            } else if ch.is_ascii_alphanumeric() || matches!(ch, '/' | ' ' | '-' | '_') {
                self.input.insert_char(ch);
            }
            return;
        }

        if self.screen == Screen::Settings {
            return;
        }

        match ch {
            '1' => self.switch_screen(Screen::Battle),
            '2' => self.switch_screen(Screen::Settings),
            _ => {}
        }
    }

    fn settings_input_char(&mut self, ch: char) {
        if ch.is_ascii_digit()
            || ch.is_ascii()
            || matches!(ch, ' ' | '\\' | '/' | ':' | '.' | '-' | '_')
        {
            self.input.insert_char(ch);
        }
    }

    fn command_backspace(&mut self) {
        if self.screen == Screen::Settings && matches!(self.focus, Focus::CommandInput) {
            self.input.backspace();
            return;
        }
        if matches!(self.focus, Focus::CommandInput) {
            self.input.backspace();
        }
    }

    fn submit_command(&mut self) {
        if self.screen == Screen::Settings && matches!(self.focus, Focus::CommandInput) {
            self.submit_settings_text_field();
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
        self.input.commit_command_history(raw.trim());
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
                self.reset_analysis_tracking();
                self.refresh_engine_after_position_change();
                let side = match self.game.side_to_move {
                    Side::Red => "红",
                    Side::Black => "黑",
                };
                let shown = self.game.last_move_uci.as_deref().unwrap_or(uci);
                self.status = format!("已走 {shown}，轮到{side}方。");
            }
            Err(err) => self.status = err.message(),
        }
    }

    /// 浏览历史：同步盘面/PV，不重新判定终局（终局标记保留）。
    fn refresh_history_view(&mut self) {
        if self.game.is_game_over() {
            self.services.engine.stop_stream();
            return;
        }
        self.tick_engine_stream();
    }

    fn refresh_engine_after_position_change(&mut self) {
        self.on_position_changed();
        if self.game.is_game_over() {
            return;
        }
        if self.game.realtime_eval || self.game.query_mode {
            self.services.engine.stop_stream();
            self.last_analysis_revision = 0;
            self.last_book_fen.clear();
            self.book_blocks_engine = false;
        }
        self.tick_engine_stream();
    }

    fn execute_slash_command(&mut self, command: SlashCommand, raw: &str) {
        match command {
            SlashCommand::New => {
                self.start_new_game();
            }
            SlashCommand::Stop => {
                self.stop_all_activity();
                self.status = "已停止：模式、引擎流与自动走子（当前局面不变）。".to_string();
            }
            SlashCommand::Help => {
                self.help_open = true;
                self.status = "操作说明（Esc 关闭）。".to_string();
            }
            SlashCommand::Undo => {
                let was_over = self.game.is_game_over();
                if GameService::undo(&mut self.game) {
                    self.refresh_engine_after_position_change();
                    self.status = if was_over && !self.game.is_game_over() {
                        "已悔棋离开终局，可继续对弈（分析/电脑模式需手动重新开启）。"
                            .to_string()
                    } else {
                        "已悔棋。".to_string()
                    };
                } else {
                    self.status = "无法悔棋（已在初始局面）。".to_string();
                }
            }
            SlashCommand::Prev => {
                if GameService::go_prev(&mut self.game) {
                    self.refresh_history_view();
                    self.status = self.history_step_status(true);
                } else {
                    self.status = "已在第一步。".to_string();
                }
            }
            SlashCommand::Next => {
                if GameService::go_next(&mut self.game) {
                    self.refresh_history_view();
                    self.status = self.history_step_status(false);
                } else {
                    self.status = "已在最新步。".to_string();
                }
            }
            SlashCommand::RedAi => {
                if !self.game.red_ai && self.game.is_game_over() {
                    self.status = "对局已结束，请先 /new 开新局。".to_string();
                } else {
                    self.game.red_ai = !self.game.red_ai;
                    self.status =
                        format!("红AI：{}", if self.game.red_ai { "开启" } else { "关闭" });
                }
            }
            SlashCommand::BlackAi => {
                if !self.game.black_ai && self.game.is_game_over() {
                    self.status = "对局已结束，请先 /new 开新局。".to_string();
                } else {
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
            }
            SlashCommand::Query => {
                if !self.game.query_mode && self.game.is_game_over() {
                    self.status = "对局已结束，请先 /new 开新局。".to_string();
                } else {
                    self.game.query_mode = !self.game.query_mode;
                    self.refresh_engine_after_mode_change();
                }
            }
            SlashCommand::Rotate => {
                self.game.rotated = !self.game.rotated;
                self.refresh_view_after_rotate();
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
                if !self.game.realtime_eval && self.game.is_game_over() {
                    self.status = "对局已结束，请先 /new 开新局。".to_string();
                } else {
                    self.game.realtime_eval = !self.game.realtime_eval;
                    self.refresh_engine_after_mode_change();
                }
            }
            SlashCommand::CopyFen => {
                self.copy_fen_to_clipboard();
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

    fn focus_settings_field(&mut self, field: SettingsField) {
        self.focus = Focus::SettingsField(field);
        self.settings_field = field;
        self.status = ui::settings_form::settings_hint(field);
    }

    fn begin_settings_input(&mut self) {
        self.focus = Focus::CommandInput;
        self.input.set_text(self.settings_field_value_string());
        self.status = format!(
            "编辑「{}」：Enter 保存，Esc 返回。",
            self.settings_field.label()
        );
    }

    fn settings_field_value_string(&self) -> String {
        match self.settings_field {
            SettingsField::EnginePath => self.engine.path.clone(),
            SettingsField::BookLocalPath => self.book.local_path.clone(),
            SettingsField::EngineProtocol => match self.engine.protocol {
                crate::engine::EngineProtocol::Uci => "uci".to_string(),
                crate::engine::EngineProtocol::Ucci => "ucci".to_string(),
            },
            SettingsField::EngineThreads => self.engine.threads.to_string(),
            SettingsField::EngineHashMb => self.engine.hash_mb.to_string(),
            SettingsField::EngineSkill => self.engine.skill_level.to_string(),
            SettingsField::EngineMultiPv => self.engine.multi_pv.to_string(),
            SettingsField::BookLocalEnabled => {
                if self.book.local_enabled {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            SettingsField::BookCloudEnabled => {
                if self.book.cloud_enabled {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            SettingsField::BookPickMode => self.book.pick_mode.clone(),
            SettingsField::BookMaxHalfmoves => self.book.max_halfmoves.to_string(),
        }
    }

    fn settings_enter_field(&mut self) {
        let field = self.settings_field;
        match field.kind() {
            SettingsFieldKind::Text | SettingsFieldKind::Number | SettingsFieldKind::Cycle => {
                self.begin_settings_input();
            }
            SettingsFieldKind::Bool => self.settings_toggle_field(),
        }
    }

    fn settings_toggle_field(&mut self) {
        let field = self.settings_field;
        match field {
            SettingsField::BookLocalEnabled => {
                self.book.local_enabled = !self.book.local_enabled;
                let _ = settings_config::save_book_flags(
                    self.book.local_enabled,
                    self.book.cloud_enabled,
                );
                self.status = format!(
                    "本地库：{}",
                    if self.book.local_enabled {
                        "开启"
                    } else {
                        "关闭"
                    }
                );
                self.after_book_settings_changed();
            }
            SettingsField::BookCloudEnabled => {
                self.book.cloud_enabled = !self.book.cloud_enabled;
                let _ = settings_config::save_book_flags(
                    self.book.local_enabled,
                    self.book.cloud_enabled,
                );
                self.status = format!(
                    "云库：{}",
                    if self.book.cloud_enabled {
                        "开启"
                    } else {
                        "关闭"
                    }
                );
                self.after_book_settings_changed();
            }
            _ => self.settings_adjust_field(1),
        }
    }

    fn settings_adjust_field(&mut self, delta: isize) {
        let field = self.settings_field;
        match field {
            SettingsField::EngineProtocol => {
                self.engine.protocol = cycle_protocol(self.engine.protocol, delta);
                let _ = settings_config::save_engine_protocol(self.engine.protocol);
                self.after_engine_settings_changed();
                self.status = format!("协议：{}", self.engine.protocol.label());
            }
            SettingsField::EngineThreads => {
                let next = clamp_threads(i32::from(self.engine.threads) + delta as i32);
                self.engine.threads = next;
                let _ = settings_config::save_engine_threads(next);
                self.after_engine_settings_changed();
                self.status = format!("线程数：{next}");
            }
            SettingsField::EngineHashMb => {
                let next = if delta == 0 {
                    self.engine.hash_mb
                } else {
                    bump_hash_mb(self.engine.hash_mb, delta)
                };
                self.engine.hash_mb = next;
                let _ = settings_config::save_engine_hash_mb(next);
                self.after_engine_settings_changed();
                self.status = format!("Hash：{next} MB");
            }
            SettingsField::EngineSkill => {
                let next = (i32::from(self.engine.skill_level) + delta as i32).clamp(0, 20) as u8;
                self.engine.skill_level = next;
                let _ = settings_config::save_engine_skill(next);
                self.after_engine_settings_changed();
                self.status = format!("Skill：{next}");
            }
            SettingsField::EngineMultiPv => {
                let next = (i32::from(self.engine.multi_pv) + delta as i32).clamp(1, 5) as u8;
                self.engine.multi_pv = next;
                let _ = settings_config::save_engine_multi_pv(next);
                self.after_engine_settings_changed();
                self.status = format!("MultiPV：{next}");
            }
            SettingsField::BookPickMode => {
                self.book.pick_mode = cycle_pick_mode(&self.book.pick_mode, delta);
                let _ = settings_config::save_book_pick_mode(&self.book.pick_mode);
                self.after_book_settings_changed();
                self.status = format!(
                    "库招：{}",
                    settings_field::pick_mode_label(&self.book.pick_mode)
                );
            }
            SettingsField::BookMaxHalfmoves => {
                let next = (i32::from(self.book.max_halfmoves) + delta as i32).clamp(0, 200) as u16;
                self.book.max_halfmoves = next;
                let _ = settings_config::save_book_max_halfmoves(next);
                self.after_book_settings_changed();
                self.status = format!("开局库步数上限：{next}");
            }
            _ => {}
        }
    }

    fn submit_settings_text_field(&mut self) {
        let field = self.settings_field;
        let value = self.input.take_text();
        let value = value.trim();
        let err_msg = match field {
            SettingsField::EnginePath => {
                self.engine.path = value.to_string();
                settings_config::save_engine_path(&self.engine.path)
                    .err()
                    .map(|e| e.to_string())
            }
            SettingsField::BookLocalPath => {
                self.book.local_path = value.to_string();
                settings_config::save_book_local_path(&self.book.local_path)
                    .err()
                    .map(|e| e.to_string())
            }
            SettingsField::EngineProtocol => {
                let proto = match value.to_ascii_lowercase().as_str() {
                    "ucci" => crate::engine::EngineProtocol::Ucci,
                    "uci" | "" => crate::engine::EngineProtocol::Uci,
                    _ => {
                        self.status = "协议请填 uci 或 ucci。".to_string();
                        self.begin_settings_input();
                        return;
                    }
                };
                self.engine.protocol = proto;
                settings_config::save_engine_protocol(proto)
                    .err()
                    .map(|e| e.to_string())
            }
            SettingsField::EngineThreads => match value.parse::<u8>() {
                Ok(v) => {
                    let v = clamp_threads(i32::from(v));
                    self.engine.threads = v;
                    settings_config::save_engine_threads(v)
                        .err()
                        .map(|e| e.to_string())
                }
                Err(_) => {
                    self.status = "线程数无效。".to_string();
                    self.begin_settings_input();
                    return;
                }
            },
            SettingsField::EngineHashMb => match value.parse::<u32>() {
                Ok(v) => {
                    let v = v.clamp(64, 8192);
                    self.engine.hash_mb = v;
                    settings_config::save_engine_hash_mb(v)
                        .err()
                        .map(|e| e.to_string())
                }
                Err(_) => {
                    self.status = "Hash 无效。".to_string();
                    self.begin_settings_input();
                    return;
                }
            },
            SettingsField::EngineSkill => match value.parse::<u8>() {
                Ok(v) => {
                    let v = v.min(20);
                    self.engine.skill_level = v;
                    settings_config::save_engine_skill(v)
                        .err()
                        .map(|e| e.to_string())
                }
                Err(_) => {
                    self.status = "Skill 无效。".to_string();
                    self.begin_settings_input();
                    return;
                }
            },
            SettingsField::EngineMultiPv => match value.parse::<u8>() {
                Ok(v) => {
                    let v = v.clamp(1, 5);
                    self.engine.multi_pv = v;
                    settings_config::save_engine_multi_pv(v)
                        .err()
                        .map(|e| e.to_string())
                }
                Err(_) => {
                    self.status = "MultiPV 无效。".to_string();
                    self.begin_settings_input();
                    return;
                }
            },
            SettingsField::BookLocalEnabled | SettingsField::BookCloudEnabled => {
                let on = matches!(value, "1" | "true" | "yes" | "on");
                if field == SettingsField::BookLocalEnabled {
                    self.book.local_enabled = on;
                } else {
                    self.book.cloud_enabled = on;
                }
                settings_config::save_book_flags(self.book.local_enabled, self.book.cloud_enabled)
                    .err()
                    .map(|e| e.to_string())
            }
            SettingsField::BookPickMode => {
                let mode = if value == "positive_random" {
                    "positive_random"
                } else {
                    "optimal"
                };
                self.book.pick_mode = mode.to_string();
                settings_config::save_book_pick_mode(mode)
                    .err()
                    .map(|e| e.to_string())
            }
            SettingsField::BookMaxHalfmoves => match value.parse::<u16>() {
                Ok(v) => {
                    self.book.max_halfmoves = v;
                    settings_config::save_book_max_halfmoves(v)
                        .err()
                        .map(|e| e.to_string())
                }
                Err(_) => {
                    self.status = "步数无效。".to_string();
                    self.begin_settings_input();
                    return;
                }
            },
        };
        if let Some(err) = err_msg {
            self.status = format!("保存失败：{err}");
            self.begin_settings_input();
            return;
        }
        match field {
            SettingsField::EnginePath
            | SettingsField::EngineProtocol
            | SettingsField::EngineThreads
            | SettingsField::EngineHashMb
            | SettingsField::EngineSkill
            | SettingsField::EngineMultiPv => self.after_engine_settings_changed(),
            _ => self.after_book_settings_changed(),
        }
        self.status = format!("已保存：{}", field.label());
        self.focus = Focus::SettingsField(field);
    }

    fn after_engine_settings_changed(&mut self) {
        self.services.engine.stop_stream();
        self.reset_analysis_tracking();
        self.refresh_engine_after_mode_change();
    }

    fn after_book_settings_changed(&mut self) {
        self.reset_analysis_tracking();
        self.tick_engine_stream();
    }

    fn switch_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.input.clear();
        self.focus = match screen {
            Screen::Battle => Focus::Board,
            Screen::Settings => Focus::SettingsField(self.settings_field),
        };
        if screen == Screen::Settings {
            self.focus_settings_field(self.settings_field);
        } else {
            self.status = "对弈：Tab 切设置；棋盘方向键+空格；/ 命令（Tab/→ 补全）。".to_string();
        }
    }

    fn activate_battle_button(&mut self, button: BattleButton) {
        if button.is_disabled(self) {
            if let Some(reason) = button.disabled_reason(self) {
                self.status = reason.to_string();
            }
            return;
        }
        match button {
            BattleButton::RedAi => {
                self.game.red_ai = !self.game.red_ai;
                self.status = format!("红电脑：{}", if self.game.red_ai { "开启" } else { "关闭" });
                if self.game.red_ai
                    && !self.game.is_game_over()
                    && self.game.side_to_move == Side::Red
                    && !self.game.query_mode
                {
                    self.status.push_str("（思考中…）");
                }
            }
            BattleButton::BlackAi => {
                self.game.black_ai = !self.game.black_ai;
                self.status = format!(
                    "黑电脑：{}",
                    if self.game.black_ai {
                        "开启"
                    } else {
                        "关闭"
                    }
                );
                if self.game.black_ai
                    && !self.game.is_game_over()
                    && self.game.side_to_move == Side::Black
                    && !self.game.query_mode
                {
                    self.status.push_str("（思考中…）");
                }
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
                self.start_new_game();
            }
            BattleButton::Undo => {
                let was_over = self.game.is_game_over();
                if GameService::undo(&mut self.game) {
                    self.refresh_engine_after_position_change();
                    self.status = if was_over && !self.game.is_game_over() {
                        "已悔棋离开终局，可继续对弈（分析/电脑模式需手动重新开启）。"
                            .to_string()
                    } else {
                        "已悔棋。".to_string()
                    };
                } else {
                    self.status = "无法悔棋。".to_string();
                }
            }
            BattleButton::RotateBoard => {
                self.game.rotated = !self.game.rotated;
                self.refresh_view_after_rotate();
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
                    self.refresh_history_view();
                    self.status = self.history_step_status(true);
                } else {
                    self.status = "已在第一步。".to_string();
                }
            }
            BattleButton::NextMove => {
                if GameService::go_next(&mut self.game) {
                    self.refresh_history_view();
                    self.status = self.history_step_status(false);
                } else {
                    self.status = "已在最新步。".to_string();
                }
            }
            BattleButton::CopyFen => {
                self.copy_fen_to_clipboard();
            }
            BattleButton::PasteFen => {
                self.status = "在 C 区输入：/pastefen <FEN>".to_string();
            }
        }
    }
}
