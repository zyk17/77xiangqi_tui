use crate::engine::AnalysisSnapshot;
use crate::game::{BoardArrow, GameState};
use crate::xiangqi::{
    parse_uci_coords, try_apply_fully_legal_uci, uci_from_coords, Board90, Side, STARTPOS_FEN,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyMoveError {
    IllegalMove(String),
}

impl ApplyMoveError {
    pub fn message(&self) -> String {
        match self {
            Self::IllegalMove(uci) => format!("非法着法：{uci}。"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GameService;

impl GameService {
    pub fn apply_uci(game: &mut GameState, uci: &str) -> Result<(), ApplyMoveError> {
        let uci = uci.trim().to_ascii_lowercase();
        let fen = game.board.to_fen(game.side_to_move);
        let Some(next_fen) = try_apply_fully_legal_uci(&fen, &uci) else {
            return Err(ApplyMoveError::IllegalMove(uci));
        };
        let (board, side) = Board90::from_fen_with_side(&next_fen)
            .ok_or_else(|| ApplyMoveError::IllegalMove(uci.clone()))?;
        game.board = board;
        game.side_to_move = side;
        game.last_move_arrow = arrow_from_uci(&uci);
        game.last_move_uci = Some(uci);
        game.pending_arrow = None;
        game.selected_cell = None;
        Ok(())
    }

    pub fn reset(game: &mut GameState) {
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
        game.board = board;
        game.side_to_move = side;
        game.last_move_uci = None;
        game.last_move_arrow = None;
        game.pending_arrow = None;
        game.selected_cell = None;
        Ok(())
    }

    pub fn engine_fen(game: &GameState) -> String {
        game.board.to_fen(game.side_to_move)
    }

    pub fn try_click_cell(game: &mut GameState, file: u8, rank: u8) -> Option<String> {
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
    fn apply_h2e2_updates_side_and_last_move_hint() {
        let mut game = GameState::default();
        GameService::apply_uci(&mut game, "h2e2").expect("legal");
        assert_eq!(game.side_to_move, Side::Black);
        assert_eq!(game.last_move_uci.as_deref(), Some("h2e2"));
        assert!(game.last_move_arrow.is_some());
    }

    #[test]
    fn illegal_rejected() {
        let mut game = GameState::default();
        let err = GameService::apply_uci(&mut game, "a0a9").expect_err("illegal");
        assert!(matches!(err, ApplyMoveError::IllegalMove(_)));
    }

    #[test]
    fn load_invalid_fen_fails() {
        let mut game = GameState::default();
        assert!(GameService::load_fen(&mut game, "not-a-fen").is_err());
    }

    #[test]
    fn reset_clears_last_move_hint() {
        let mut game = GameState::default();
        GameService::apply_uci(&mut game, "h2e2").expect("move");
        GameService::reset(&mut game);
        assert!(game.last_move_uci.is_none());
        assert!(game.last_move_arrow.is_none());
    }
}
