use crate::{
    engine::AnalysisSnapshot,
    xiangqi::{Board90, Side},
};

mod history;

pub use history::MoveHistory;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardArrow {
    pub from_file: u8,
    pub from_rank: u8,
    pub to_file: u8,
    pub to_rank: u8,
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub board: Board90,
    pub side_to_move: Side,
    pub history: MoveHistory,
    pub red_ai: bool,
    pub black_ai: bool,
    pub query_mode: bool,
    pub realtime_eval: bool,
    pub rotated: bool,
    pub last_move_uci: Option<String>,
    pub last_move_arrow: Option<BoardArrow>,
    pub pending_arrow: Option<BoardArrow>,
    pub selected_cell: Option<(u8, u8)>,
    pub analysis: AnalysisSnapshot,
    /// 本盘终局说明（将死/困毙等）；非空时停模式/引擎/自动走子，可浏览棋谱，不会自动新局。
    pub game_over: Option<String>,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            board: Board90::startpos(),
            side_to_move: Side::Red,
            history: MoveHistory::new_game(),
            red_ai: false,
            black_ai: false,
            query_mode: false,
            realtime_eval: false,
            rotated: false,
            last_move_uci: None,
            last_move_arrow: None,
            pending_arrow: None,
            selected_cell: None,
            analysis: AnalysisSnapshot::idle(),
            game_over: None,
        }
    }
}

impl GameState {
    pub fn is_game_over(&self) -> bool {
        self.game_over.is_some()
    }
}
