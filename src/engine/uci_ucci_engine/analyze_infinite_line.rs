//! `go infinite` 单行 stdout 合并到状态并更新共享快照。

use super::info_state::{EngineInfoState, apply_parsed_info_to_state};
use crate::engine::analysis_store::EngineAnalysisStore;
use crate::engine::protocol::parse_uci_style_info_tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InfiniteLineOutcome {
    Continue,
    GotBestmove,
}

pub(crate) fn apply_infinite_stdout_line(
    line: &str,
    _fen: &str,
    st: &mut EngineInfoState,
) -> InfiniteLineOutcome {
    let line = line.trim();
    if line.starts_with("info ") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(parsed) = parse_uci_style_info_tokens(&parts) {
            apply_parsed_info_to_state(&parsed, st);
        }
        InfiniteLineOutcome::Continue
    } else if line.starts_with("bestmove") {
        let tok: Vec<&str> = line.split_whitespace().collect();
        if tok.len() >= 2 {
            st.best_move = tok[1].to_string();
        }
        InfiniteLineOutcome::GotBestmove
    } else {
        InfiniteLineOutcome::Continue
    }
}

pub(crate) fn patch_store_from_state(
    store: &mut EngineAnalysisStore,
    fen: &str,
    st: &EngineInfoState,
) {
    store.patch_from_info_state(fen, st);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_line_updates_state_best_move() {
        let mut st = EngineInfoState::new();
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        let out = apply_infinite_stdout_line("info depth 10 score cp 25 pv h2e2", fen, &mut st);
        assert_eq!(out, InfiniteLineOutcome::Continue);
        let mut store = EngineAnalysisStore::empty_for_fen(fen);
        patch_store_from_state(&mut store, fen, &st);
        assert_eq!(store.result.best_move, "h2e2");
    }

    #[test]
    fn bestmove_line_sets_state_and_ends() {
        let mut st = EngineInfoState::new();
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        let out = apply_infinite_stdout_line("bestmove h2e2", fen, &mut st);
        assert_eq!(out, InfiniteLineOutcome::GotBestmove);
        assert_eq!(st.best_move, "h2e2");
    }
}
