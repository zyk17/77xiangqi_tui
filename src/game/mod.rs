use crate::{
    engine::AnalysisSnapshot,
    xiangqi::{Board90, Side},
};

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
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            board: Board90::startpos(),
            side_to_move: Side::Red,
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
        }
    }
}

impl GameState {
    pub fn reset(&mut self) {
        crate::service::GameService::reset(self);
    }

    pub fn active_mode_count(&self) -> usize {
        [
            self.red_ai,
            self.black_ai,
            self.query_mode,
            self.realtime_eval,
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
    }
}
