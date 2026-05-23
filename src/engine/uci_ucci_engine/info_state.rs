//! 流式 `info` 行合并到 [`EngineInfoState`]，以及主变挑选。

use crate::engine::analysis_types::EngineInfoCandidate;
use crate::engine::protocol::{candidate_from_parsed, ParsedUciStyleInfo};
use std::collections::BTreeMap;

pub(crate) struct EngineInfoState {
    pub best_move: String,
    pub score: f64,
    pub pv: Vec<String>,
    pub depth_seen: Option<i32>,
    pub cands_by_rank: BTreeMap<i32, EngineInfoCandidate>,
    pub search_time_ms: Option<u64>,
    pub nps: Option<u64>,
    pub nodes: Option<u64>,
    pub wdl: Option<[u64; 3]>,
    pub mate: Option<i32>,
}

impl EngineInfoState {
    pub(crate) fn new() -> Self {
        Self {
            best_move: "stub_move".to_string(),
            score: 0.0,
            pv: vec![],
            depth_seen: None,
            cands_by_rank: BTreeMap::new(),
            search_time_ms: None,
            nps: None,
            nodes: None,
            wdl: None,
            mate: None,
        }
    }
}

pub(crate) fn apply_parsed_info_to_state(parsed: &ParsedUciStyleInfo, st: &mut EngineInfoState) {
    if let Some(t) = parsed.search_time_ms {
        st.search_time_ms = Some(t);
    }
    if let Some(n) = parsed.nps {
        st.nps = Some(n);
    }
    if let Some(node_count) = parsed.nodes {
        st.nodes = Some(node_count);
    }
    if let Some(parsed_wdl) = parsed.wdl {
        st.wdl = Some(parsed_wdl);
    }
    if parsed.has_score {
        st.mate = parsed.mate;
    }
    st.score = parsed.cand_score;
    if !parsed.pv_tok.is_empty() {
        st.pv.clone_from(&parsed.pv_tok);
    }
    st.depth_seen = parsed.depth;
    let prev = st.cands_by_rank.get(&parsed.multipv).cloned();
    st.cands_by_rank.insert(
        parsed.multipv,
        candidate_from_parsed(parsed, &st.best_move, prev.as_ref()),
    );
}

pub(crate) fn select_main_line_from_candidates(
    cand_list: &[EngineInfoCandidate],
    fallback_best_move: &str,
    fallback_score: f64,
    fallback_pv: &[String],
    fallback_depth: Option<i32>,
    fallback_mate: Option<i32>,
) -> (String, f64, Vec<String>, Option<i32>, Option<i32>) {
    if let Some(first) = cand_list.first() {
        let main_bm = first.best_move.clone();
        let main_sc = first.score;
        let main_mate = first.mate;
        let main_pv = first.pv.clone();
        let main_d = first.depth;
        return (main_bm, main_sc, main_pv, main_d, main_mate);
    }
    (
        fallback_best_move.to_string(),
        fallback_score,
        fallback_pv.to_vec(),
        fallback_depth,
        fallback_mate,
    )
}

/// 与 `^[a-i][0-9][a-i][0-9]$` 一致。
pub(crate) fn uci_xiangqi_best_ready(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let b = s.as_bytes();
    if b.len() != 4 {
        return false;
    }
    (b[0] >= b'a' && b[0] <= b'i')
        && (b[1] >= b'0' && b[1] <= b'9')
        && (b[2] >= b'a' && b[2] <= b'i')
        && (b[3] >= b'0' && b[3] <= b'9')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::analysis_types::EngineInfoCandidate;

    #[test]
    fn uci_xiangqi_best_ready_accepts_valid_iccs_uci() {
        assert!(uci_xiangqi_best_ready("h2e2"));
        assert!(uci_xiangqi_best_ready("a0a9"));
    }

    #[test]
    fn uci_xiangqi_best_ready_rejects_stub_and_invalid() {
        assert!(!uci_xiangqi_best_ready("stub_move"));
        assert!(!uci_xiangqi_best_ready("j2e2"));
        assert!(!uci_xiangqi_best_ready("e2e10"));
        assert!(!uci_xiangqi_best_ready(""));
    }

    #[test]
    fn select_main_line_prefers_first_candidate() {
        let cands = vec![EngineInfoCandidate {
            rank: 1,
            best_move: "h2e2".to_string(),
            score: 30.0,
            depth: Some(8),
            pv: vec!["h2e2".to_string()],
            ..EngineInfoCandidate::default()
        }];
        let (bm, sc, pv, d, mate) =
            select_main_line_from_candidates(&cands, "stub_move", 0.0, &[], None, None);
        assert_eq!(bm, "h2e2");
        assert_eq!(sc, 30.0);
        assert_eq!(pv, vec!["h2e2".to_string()]);
        assert_eq!(d, Some(8));
        assert!(mate.is_none());
    }
}
