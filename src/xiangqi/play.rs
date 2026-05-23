use super::board::Board90;
use super::rules::uci_is_fully_legal;

/// 全规则合法则走子并返回新 FEN（轮到对方，`w/b` + `0 1` 计数）。
pub fn try_apply_fully_legal_uci(fen: &str, uci: &str) -> Option<String> {
    let fen = fen.trim();
    if !uci_is_fully_legal(fen, uci) {
        return None;
    }
    let (mut board, side) = Board90::from_fen_with_side(fen)?;
    if !board.apply_uci(uci) {
        return None;
    }
    Some(board.to_fen(side.other()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

    #[test]
    fn legal_opening_cannon() {
        let next = try_apply_fully_legal_uci(START, "h2e2").expect("h2e2");
        assert!(next.contains(" b "));
    }

    #[test]
    fn illegal_move_rejected() {
        assert!(try_apply_fully_legal_uci(START, "a0a9").is_none());
    }
}
