//! 局面终局判定（对齐 GUI `usePlayPageBoardPositionBundle`）。

use super::{Side, board::Board90};

/// 当前行棋方无合法着时返回终局说明；否则 `None`。
pub fn game_over_message(board: &Board90, side_to_move: Side) -> Option<String> {
    let side_red = side_to_move.is_red();
    let defender = if side_red { "红方" } else { "黑方" };
    let winner = if side_red { "黑方" } else { "红方" };

    if !board.has_king(side_red) {
        return Some(format!("{defender}已无将/帅，{winner}胜"));
    }
    if !board.legal_ucis_for_side(side_to_move).is_empty() {
        return None;
    }
    if board.in_check(side_to_move) {
        return Some(format!("{defender}被将死，{winner}胜"));
    }
    Some(format!("{defender}困毙，{winner}胜"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xiangqi::{Board90, try_apply_fully_legal_uci};

    const START: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

    #[test]
    fn startpos_not_game_over() {
        let board = Board90::from_fen(START).expect("fen");
        assert!(game_over_message(&board, Side::Red).is_none());
    }

    #[test]
    fn missing_king_is_game_over() {
        let fen = "4k4/9/9/9/9/9/9/9/9/9 w - - 0 1";
        let board = Board90::from_fen(fen).expect("fen");
        let msg = game_over_message(&board, Side::Red).expect("over");
        assert!(msg.contains("已无将"));
    }

    #[test]
    fn after_legal_move_play_continues() {
        let next = try_apply_fully_legal_uci(START, "h2e2").expect("move");
        let board = Board90::from_fen(&next).expect("fen");
        assert!(game_over_message(&board, Side::Black).is_none());
    }
}
