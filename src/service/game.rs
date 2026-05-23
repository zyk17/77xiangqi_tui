use crate::engine::AnalysisSnapshot;
use crate::game::{BoardArrow, GameState};
use crate::xiangqi::{
    parse_uci_coords, try_apply_fully_legal_uci, uci_from_coords, Board90, Side, STARTPOS_FEN,
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
        game.history.push_move(next_fen, uci);
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
        let (board, side) = Board90::from_fen_with_side(fen)
            .ok_or_else(|| "无法解析 FEN。".to_string())?;
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
        if game.board.is_empty(file, rank) {
            let Some((from_file, from_rank)) = game.selected_cell else {
                return None;
            };
            let uci = uci_from_coords(
                from_rank as usize,
                from_file as usize,
                rank as usize,
                file as usize,
            );
            game.selected_cell = None;
            return Some(uci);
        }
        if game.board.is_red_piece(file, rank) != game.side_to_move.is_red() {
            game.selected_cell = None;
            return None;
        }
        game.selected_cell = Some((file, rank));
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
    fn reset_clears_analysis_source_to_startpos() {
        let mut game = GameState::default();
        GameService::apply_uci(&mut game, "h2e2").expect("move");
        GameService::reset(&mut game);
        assert_eq!(game.analysis.source, STARTPOS_FEN);
    }
}
