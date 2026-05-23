use crate::{
    engine::AnalysisSnapshot,
    xiangqi::{Board90, STARTPOS_FEN},
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
    pub red_ai: bool,
    pub black_ai: bool,
    pub query_mode: bool,
    pub realtime_eval: bool,
    pub rotated: bool,
    pub last_move_uci: Option<String>,
    pub pending_arrow: Option<BoardArrow>,
    pub analysis: AnalysisSnapshot,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            board: Board90::startpos(),
            red_ai: false,
            black_ai: false,
            query_mode: false,
            realtime_eval: true,
            rotated: false,
            last_move_uci: None,
            pending_arrow: None,
            analysis: AnalysisSnapshot {
                time_text: "0.25s".to_string(),
                depth: 17,
                nps: 15744,
                nodes: 3_920_000,
                score_text: "+6".to_string(),
                best_move: "车八进九".to_string(),
                win_rate_text: "50.9%/49.1%".to_string(),
                pv: vec![
                    "兵七进一".to_string(),
                    "炮2平三".to_string(),
                    "车9进1".to_string(),
                ],
                source: "skeleton".to_string(),
            },
        }
    }
}

impl GameState {
    pub fn reset(&mut self) {
        self.board = Board90::startpos();
        self.last_move_uci = None;
        self.pending_arrow = None;
        self.analysis = AnalysisSnapshot {
            source: STARTPOS_FEN.to_string(),
            ..AnalysisSnapshot::idle()
        };
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
