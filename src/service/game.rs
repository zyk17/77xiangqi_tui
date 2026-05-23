use crate::engine::{AnalysisSnapshot, pv_ui::truncate_engine_pv_for_ui};
use crate::game::{BoardArrow, GameState};
use crate::xiangqi::{
    Board90, STARTPOS_FEN, Side, parse_uci_coords, try_apply_fully_legal_uci, uci_from_coords,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyMoveError {
    IllegalMove(String),
    BrowseOnly,
}

impl ApplyMoveError {
    pub fn message(&self) -> String {
        match self {
            Self::IllegalMove(uci) => format!("非法着法：{uci}。"),
            Self::BrowseOnly => "正在浏览历史，请 /next 回到最新步或走新着以截断。".to_string(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GameService;

impl GameService {
    pub fn apply_uci(game: &mut GameState, uci: &str) -> Result<(), ApplyMoveError> {
        let uci = uci.trim().to_ascii_lowercase();
        if !game.history.at_head() {
            return Err(ApplyMoveError::BrowseOnly);
        }
        let fen = game.history.current_fen();
        let Some(next_fen) = try_apply_fully_legal_uci(fen, &uci) else {
            return Err(ApplyMoveError::IllegalMove(uci));
        };
        let pv = truncate_engine_pv_for_ui(&game.analysis.pv);
        game.history.push_move(next_fen, uci, pv);
        Self::sync_from_history(game);
        game.pending_arrow = None;
        game.selected_cell = None;
        Ok(())
    }

    pub fn sync_from_history(game: &mut GameState) {
        if let Some((board, side)) = game.history.load_current() {
            game.board = board;
            game.side_to_move = side;
        }
        Self::sync_last_move_from_history(game);
        Self::sync_pv_for_view(game);
    }

    fn sync_pv_for_view(game: &mut GameState) {
        if game.history.at_head() {
            return;
        }
        game.analysis.pv = game.history.pv_at_view().to_vec();
    }

    fn sync_last_move_from_history(game: &mut GameState) {
        if let Some(uci) = game.history.last_move_uci_at_view() {
            game.last_move_uci = Some(uci.to_string());
            game.last_move_arrow = arrow_from_uci(uci);
        } else {
            game.last_move_uci = None;
            game.last_move_arrow = None;
        }
    }

    pub fn sync_view_after_rotate(game: &mut GameState) {
        Self::sync_last_move_from_history(game);
        game.selected_cell = None;
    }

    pub fn reset(game: &mut GameState) {
        game.history = crate::game::MoveHistory::new_game();
        game.board = Board90::startpos();
        game.side_to_move = Side::Red;
        game.last_move_uci = None;
        game.last_move_arrow = None;
        game.pending_arrow = None;
        game.selected_cell = None;
        game.analysis = AnalysisSnapshot {
            source: STARTPOS_FEN.to_string(),
            ..AnalysisSnapshot::idle()
        };
    }

    pub fn load_fen(game: &mut GameState, fen: &str) -> Result<(), String> {
        let fen = fen.trim();
        let (board, side) =
            Board90::from_fen_with_side(fen).ok_or_else(|| "无法解析 FEN。".to_string())?;
        let normalized = board.to_fen(side);
        game.history.reset_to_fen(normalized);
        game.board = board;
        game.side_to_move = side;
        game.last_move_uci = None;
        game.last_move_arrow = None;
        game.pending_arrow = None;
        game.selected_cell = None;
        Ok(())
    }

    pub fn undo(game: &mut GameState) -> bool {
        if !game.history.undo() {
            return false;
        }
        Self::sync_from_history(game);
        game.pending_arrow = None;
        game.selected_cell = None;
        true
    }

    pub fn go_prev(game: &mut GameState) -> bool {
        if !game.history.go_prev() {
            return false;
        }
        Self::sync_from_history(game);
        game.pending_arrow = None;
        game.selected_cell = None;
        true
    }

    pub fn go_next(game: &mut GameState) -> bool {
        if !game.history.go_next() {
            return false;
        }
        Self::sync_from_history(game);
        game.pending_arrow = None;
        game.selected_cell = None;
        true
    }

    pub fn engine_fen(game: &GameState) -> String {
        game.board.to_fen(game.side_to_move)
    }

    pub fn try_click_cell(game: &mut GameState, file: u8, rank: u8) -> Option<String> {
        if !game.history.at_head() {
            return None;
        }

        let side = game.side_to_move;

        if let Some((from_file, from_rank)) = game.selected_cell {
            if (from_file, from_rank) == (file, rank) {
                return None;
            }
            // 已选中时再点己方棋子：只改选，不生成走子 UCI
            if game.board.is_own_for(file, rank, side) {
                game.selected_cell = Some((file, rank));
                return None;
            }
            game.selected_cell = None;
            return Some(uci_from_coords(
                from_rank as usize,
                from_file as usize,
                rank as usize,
                file as usize,
            ));
        }

        if game.board.is_own_for(file, rank, side) {
            game.selected_cell = Some((file, rank));
        }
        None
    }
}

pub fn arrow_from_uci(uci: &str) -> Option<BoardArrow> {
    let (r1, c1, r2, c2) = parse_uci_coords(uci)?;
    Some(BoardArrow {
        from_file: c1 as u8,
        from_rank: r1 as u8,
        to_file: c2 as u8,
        to_rank: r2 as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;

    #[test]
    fn apply_h2e2_updates_side() {
        let mut game = GameState::default();
        GameService::apply_uci(&mut game, "h2e2").expect("legal");
        assert_eq!(game.side_to_move, Side::Black);
        assert_eq!(game.last_move_uci.as_deref(), Some("h2e2"));
    }

    #[test]
    fn illegal_rejected() {
        let mut game = GameState::default();
        let err = GameService::apply_uci(&mut game, "a0a9").expect_err("illegal");
        assert!(matches!(err, ApplyMoveError::IllegalMove(_)));
    }

    #[test]
    fn undo_clears_last_move_display() {
        let mut game = GameState::default();
        GameService::apply_uci(&mut game, "h2e2").expect("move");
        assert!(GameService::undo(&mut game));
        assert!(game.last_move_uci.is_none());
        assert!(game.last_move_arrow.is_none());
    }

    #[test]
    fn browse_only_blocks_move() {
        let mut game = GameState::default();
        GameService::apply_uci(&mut game, "h2e2").expect("move");
        GameService::go_prev(&mut game);
        let err = GameService::apply_uci(&mut game, "h9g7").expect_err("browse");
        assert!(matches!(err, ApplyMoveError::BrowseOnly));
    }

    #[test]
    fn load_invalid_fen_fails() {
        let mut game = GameState::default();
        assert!(GameService::load_fen(&mut game, "not-a-fen").is_err());
    }

    #[test]
    fn apply_uci_same_when_rotated() {
        let mut game = GameState {
            rotated: true,
            ..GameState::default()
        };
        GameService::apply_uci(&mut game, "h2e2").expect("move");
        assert_eq!(game.history.last_move_uci_at_view(), Some("h2e2"));
        assert_eq!(game.last_move_uci.as_deref(), Some("h2e2"));
    }

    #[test]
    fn click_enemy_with_selection_generates_capture_uci() {
        let mut game = GameState {
            selected_cell: Some((7, 7)),
            ..GameState::default()
        };
        let uci = GameService::try_click_cell(&mut game, 4, 2).expect("capture uci");
        assert_eq!(uci, "h2e7");
        assert!(game.selected_cell.is_none());
    }

    #[test]
    fn click_own_piece_reselects() {
        let mut game = GameState::default();
        GameService::try_click_cell(&mut game, 7, 7);
        GameService::try_click_cell(&mut game, 0, 9);
        assert_eq!(game.selected_cell, Some((0, 9)));
    }

    #[test]
    fn click_second_friendly_cannon_does_not_emit_uci() {
        let mut game = GameState::default();
        GameService::try_click_cell(&mut game, 7, 7);
        assert_eq!(game.selected_cell, Some((7, 7)));
        assert!(GameService::try_click_cell(&mut game, 1, 7).is_none());
        assert_eq!(game.selected_cell, Some((1, 7)));
    }

    #[test]
    fn reset_clears_analysis_source_to_startpos() {
        let mut game = GameState::default();
        GameService::apply_uci(&mut game, "h2e2").expect("move");
        GameService::reset(&mut game);
        assert_eq!(game.analysis.source, STARTPOS_FEN);
    }
}
