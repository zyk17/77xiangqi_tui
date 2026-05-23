//! 流式分析共享快照（替代 GUI 的 `analysis_store` JSON）。

use crate::engine::analysis_types::{EngineAnalyzeResult, EngineInfoCandidate};
use crate::engine::pv_ui::truncate_engine_pv_for_ui;
use crate::engine::uci_ucci_engine::info_state::{
    select_main_line_from_candidates, EngineInfoState,
};

#[derive(Debug, Clone, PartialEq)]
pub struct EngineAnalysisStore {
    pub fen: String,
    pub result: EngineAnalyzeResult,
    /// 每次 patch 递增，供 UI 跳过未变化的帧。
    pub revision: u64,
}

impl EngineAnalysisStore {
    pub fn empty_for_fen(fen: &str) -> Self {
        Self {
            fen: fen.trim().to_string(),
            result: EngineAnalyzeResult::stub(),
            revision: 0,
        }
    }

    pub fn reset_for_stream(&mut self, fen: &str) {
        *self = Self::empty_for_fen(fen);
    }

    /// 将 `info` 合并后的引擎状态写入共享快照（流式主路径）。
    pub fn patch_from_info_state(&mut self, fen: &str, st: &EngineInfoState) {
        self.fen = fen.trim().to_string();
        self.result = analyze_result_from_info_state(st);
        self.bump_revision();
    }

    pub fn patch_best_move(&mut self, best_move: String) {
        self.result.best_move = best_move;
        self.bump_revision();
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
}

pub fn analyze_result_from_info_state(st: &EngineInfoState) -> EngineAnalyzeResult {
    let candidates: Vec<EngineInfoCandidate> = st.cands_by_rank.values().cloned().collect();
    let (best_move, score, pv, depth, mate) = select_main_line_from_candidates(
        &candidates,
        &st.best_move,
        st.score,
        &st.pv,
        st.depth_seen,
        st.mate,
    );
    let score_cp = candidates
        .first()
        .and_then(|c| c.score_cp)
        .map(i64::from);
    EngineAnalyzeResult {
        best_move,
        score,
        score_cp,
        pv: truncate_engine_pv_for_ui(&pv),
        depth,
        candidates,
        search_time_ms: st.search_time_ms,
        nps: st.nps,
        nodes: st.nodes,
        wdl: st.wdl,
        mate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::protocol::parse_uci_style_info_tokens;
    use crate::engine::uci_ucci_engine::info_state::apply_parsed_info_to_state;

    #[test]
    fn patch_from_info_state_sets_best_move_and_depth() {
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        let mut st = EngineInfoState::new();
        let parts: Vec<&str> = "info depth 12 score cp 30 pv h2e2 h7e7"
            .split_whitespace()
            .collect();
        let parsed = parse_uci_style_info_tokens(&parts).expect("parse");
        apply_parsed_info_to_state(&parsed, &mut st);
        let mut store = EngineAnalysisStore::empty_for_fen(fen);
        store.patch_from_info_state(fen, &st);
        assert_eq!(store.result.best_move, "h2e2");
        assert_eq!(store.result.depth, Some(12));
        assert!(store.result.pv.len() <= 16);
    }

    #[test]
    fn reset_for_stream_clears_previous_result() {
        let mut store = EngineAnalysisStore::empty_for_fen("fen-a");
        store.result.best_move = "h2e2".to_string();
        store.revision = 3;
        store.reset_for_stream("fen-b");
        assert_eq!(store.fen, "fen-b");
        assert!(store.result.best_move.is_empty());
        assert_eq!(store.revision, 0);
    }

    #[test]
    fn patch_bumps_revision() {
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        let mut store = EngineAnalysisStore::empty_for_fen(fen);
        let mut st = EngineInfoState::new();
        store.patch_from_info_state(fen, &st);
        assert_eq!(store.revision, 1);
        let parts: Vec<&str> = "info depth 5 score cp 10 pv h2e2"
            .split_whitespace()
            .collect();
        let parsed = parse_uci_style_info_tokens(&parts).expect("parse");
        apply_parsed_info_to_state(&parsed, &mut st);
        store.patch_from_info_state(fen, &st);
        assert_eq!(store.revision, 2);
    }
}
