//! 全规则合法性（几何 + 飞将 + 照将）。

use super::board::Board90;
pub fn uci_is_fully_legal(engine_fen: &str, uci: &str) -> bool {
    let Some((board, side)) = Board90::from_fen_with_side(engine_fen.trim()) else {
        return false;
    };
    board.legal_ucis_for_side(side).iter().any(|u| u == uci)
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

    #[test]
    fn start_h2e2_legal() {
        assert!(uci_is_fully_legal(START, "h2e2"));
        assert!(!uci_is_fully_legal(START, "h2h2"));
        assert!(!uci_is_fully_legal(START, "a0a9"));
    }
}
