use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::engine::analysis_types::{EngineAnalyzeResult, EngineInfoCandidate};

use super::engine_core::UciUcciEngine;
use super::info_state::{
    EngineInfoState, apply_parsed_info_to_state, select_main_line_from_candidates,
};
#[cfg(test)]
use super::test_hook::try_test_analyze_hook;
use super::types::{EngineAnalyzeRequest, EngineStdoutPoll};
use super::ui_helpers::stub_result;
use crate::engine::protocol::parse_uci_style_info_tokens;
use crate::runtime_log;

impl UciUcciEngine {
    /// AI 自动走子：固定深度 + 时限的一次性分析。
    pub fn analyze_autoplay_once(&mut self, fen: &str) -> EngineAnalyzeResult {
        self.analyze_inner(EngineAnalyzeRequest {
            fen,
            depth: Some(12),
            movetime_ms: Some(500),
            search_moves: None,
            search_nodes: None,
            multipv_override: Some(1),
            cancel: None,
        })
    }

    fn analyze_inner(&mut self, req: EngineAnalyzeRequest<'_>) -> EngineAnalyzeResult {
        #[cfg(test)]
        if let Some(v) = try_test_analyze_hook(&req) {
            return v;
        }

        let EngineAnalyzeRequest {
            fen,
            depth,
            movetime_ms,
            search_moves,
            search_nodes,
            multipv_override,
            cancel,
        } = req;
        if self.engine_path.is_none() {
            return stub_result();
        }
        if self.rt.lock().map(|g| g.is_none()).unwrap_or(true) {
            self.start();
        }
        if self.rt.lock().map(|g| g.is_none()).unwrap_or(true) {
            return stub_result();
        }
        let _ = self.send_cmd("stop");
        let multi_pv = multipv_override.unwrap_or(1);
        if let Err(e) = self.send_cmd(&format!("setoption name MultiPV value {multi_pv}")) {
            runtime_log::warn(format!(
                "[engine_analyze] send_err stage=set_multipv err={e}"
            ));
            self.terminate_locked();
            return stub_result();
        }
        if !fen.trim().is_empty() {
            if let Err(e) = self.send_cmd(&format!("position fen {}", fen.trim())) {
                runtime_log::warn(format!(
                    "[engine_analyze] send_err stage=position_fen err={e}"
                ));
                self.terminate_locked();
                return stub_result();
            }
        } else if let Err(e) = self.send_cmd("position startpos") {
            runtime_log::warn(format!(
                "[engine_analyze] send_err stage=position_startpos err={e}"
            ));
            self.terminate_locked();
            return stub_result();
        }
        self.clear_queue();
        let mut go_suffix = String::new();
        if let Some(moves) = search_moves {
            let filtered: Vec<&str> = moves
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if !filtered.is_empty() {
                go_suffix.push_str(" searchmoves ");
                go_suffix.push_str(&filtered.join(" "));
            }
        }
        if let Some(mt) = movetime_ms.filter(|&m| m > 0) {
            if let Err(e) = self.send_cmd(&format!("go movetime {mt}{go_suffix}")) {
                runtime_log::warn(format!(
                    "[engine_analyze] send_err stage=go_movetime err={e}"
                ));
                self.terminate_locked();
                return stub_result();
            }
        } else if let Some(n) = search_nodes.filter(|&n| n > 0) {
            if let Err(e) = self.send_cmd(&format!("go nodes {n}{go_suffix}")) {
                runtime_log::warn(format!("[engine_analyze] send_err stage=go_nodes err={e}"));
                self.terminate_locked();
                return stub_result();
            }
        } else {
            let d = depth.unwrap_or(8).max(1);
            if let Err(e) = self.send_cmd(&format!("go depth {d}{go_suffix}")) {
                runtime_log::warn(format!("[engine_analyze] send_err stage=go_depth err={e}"));
                self.terminate_locked();
                return stub_result();
            }
        }
        let mut st = EngineInfoState::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut got_best = false;
        while Instant::now() < deadline && !got_best {
            if cancel
                .as_ref()
                .map(|c| c.load(Ordering::SeqCst))
                .unwrap_or(false)
            {
                let _ = self.send_cmd("stop");
                runtime_log::warn("[engine_analyze] cancelled_by_flag");
                break;
            }
            match self.poll_line(Duration::from_millis(120)) {
                EngineStdoutPoll::Disconnected { child_status } => {
                    runtime_log::warn(format!(
                        "[engine_analyze] disconnected child_status={child_status}"
                    ));
                    break;
                }
                EngineStdoutPoll::Tick => {}
                EngineStdoutPoll::Line(line) => {
                    let line = line.trim();
                    if line.starts_with("info ") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if let Some(parsed) = parse_uci_style_info_tokens(&parts) {
                            apply_parsed_info_to_state(&parsed, &mut st);
                        }
                    } else if line.starts_with("bestmove") {
                        let tok: Vec<&str> = line.split_whitespace().collect();
                        if tok.len() >= 2 {
                            st.best_move = tok[1].to_string();
                        }
                        got_best = true;
                    }
                }
            }
        }
        if !got_best {
            let _ = self.send_cmd("stop");
            runtime_log::warn("[engine_analyze] bestmove_timeout_or_disconnected; fallback_result");
        }
        let cand_list: Vec<EngineInfoCandidate> = st.cands_by_rank.values().cloned().collect();
        let mut score_cp: Option<i64> = None;
        if !cand_list.is_empty() {
            let (main_bm, main_sc, main_pv, main_d, main_mate) = select_main_line_from_candidates(
                &cand_list,
                &st.best_move,
                st.score,
                &st.pv,
                st.depth_seen,
                st.mate,
            );
            st.best_move = main_bm;
            st.score = main_sc;
            st.pv = main_pv;
            st.depth_seen = main_d;
            st.mate = main_mate;
            score_cp = cand_list[0].score_cp.map(i64::from);
        }
        EngineAnalyzeResult {
            best_move: st.best_move,
            score: st.score,
            score_cp,
            pv: st.pv,
            depth: st.depth_seen,
            candidates: cand_list,
            search_time_ms: st.search_time_ms,
            nps: st.nps,
            nodes: st.nodes,
            wdl: st.wdl,
            mate: st.mate,
        }
    }
}
