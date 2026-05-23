use crate::xiangqi::{Board90, Side, STARTPOS_FEN};

#[derive(Debug, Clone)]
pub struct MoveHistory {
    fens: Vec<String>,
    /// `moves[i]`：从 `fens[i]` 走到 `fens[i+1]` 的 UCI。
    moves: Vec<String>,
    index: usize,
}

impl Default for MoveHistory {
    fn default() -> Self {
        Self::new_game()
    }
}

impl MoveHistory {
    pub fn new_game() -> Self {
        Self {
            fens: vec![STARTPOS_FEN.to_string()],
            moves: Vec::new(),
            index: 0,
        }
    }

    pub fn current_fen(&self) -> &str {
        self.fens
            .get(self.index)
            .map(String::as_str)
            .unwrap_or(STARTPOS_FEN)
    }

    pub fn last_move_uci_at_view(&self) -> Option<&str> {
        if self.index == 0 {
            return None;
        }
        self.moves.get(self.index - 1).map(String::as_str)
    }

    pub fn can_undo(&self) -> bool {
        self.at_head() && self.fens.len() > 1
    }

    pub fn can_go_prev(&self) -> bool {
        self.index > 0
    }

    pub fn can_go_next(&self) -> bool {
        self.index + 1 < self.fens.len()
    }

    pub fn at_head(&self) -> bool {
        self.index + 1 == self.fens.len()
    }

    pub fn push_move(&mut self, fen_after: String, uci: String) {
        if self.index + 1 < self.fens.len() {
            self.fens.truncate(self.index + 1);
            self.moves.truncate(self.index);
        }
        self.moves.push(uci);
        self.fens.push(fen_after);
        self.index = self.fens.len() - 1;
    }

    /// 悔棋：仅在最新步删除最后一手（截断历史）。
    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.fens.pop();
        self.moves.pop();
        self.index = self.fens.len() - 1;
        true
    }

    /// 浏览上一步（不删后续 FEN，可用 `/next` 恢复）。
    pub fn go_prev(&mut self) -> bool {
        if !self.can_go_prev() {
            return false;
        }
        self.index -= 1;
        true
    }

    pub fn go_next(&mut self) -> bool {
        if !self.can_go_next() {
            return false;
        }
        self.index += 1;
        true
    }

    pub fn load_current(&self) -> Option<(Board90, Side)> {
        Board90::from_fen_with_side(self.current_fen())
    }

    pub fn reset_to_fen(&mut self, fen: String) {
        self.fens = vec![fen];
        self.moves.clear();
        self.index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xiangqi::try_apply_fully_legal_uci;

    #[test]
    fn undo_truncates_tail() {
        let mut h = MoveHistory::new_game();
        let next = try_apply_fully_legal_uci(h.current_fen(), "h2e2").expect("move");
        h.push_move(next, "h2e2".to_string());
        assert!(h.undo());
        assert_eq!(h.current_fen(), STARTPOS_FEN);
        assert!(h.at_head());
        assert_eq!(h.fens.len(), 1);
    }

    #[test]
    fn prev_next_browse_without_truncating() {
        let mut h = MoveHistory::new_game();
        let fen2 = try_apply_fully_legal_uci(h.current_fen(), "h2e2").expect("move");
        h.push_move(fen2.clone(), "h2e2".to_string());
        let fen3 = try_apply_fully_legal_uci(h.current_fen(), "h9g7").expect("move");
        h.push_move(fen3, "h9g7".to_string());
        assert!(h.go_prev());
        assert_eq!(h.last_move_uci_at_view(), Some("h2e2"));
        assert!(h.go_next());
        assert!(h.at_head());
        assert_eq!(h.fens.len(), 3);
    }

    #[test]
    fn last_move_follows_view_index() {
        let mut h = MoveHistory::new_game();
        let fen2 = try_apply_fully_legal_uci(h.current_fen(), "h2e2").expect("move");
        h.push_move(fen2, "h2e2".to_string());
        assert_eq!(h.last_move_uci_at_view(), Some("h2e2"));
        assert!(h.go_prev());
        assert_eq!(h.last_move_uci_at_view(), None);
    }
}
