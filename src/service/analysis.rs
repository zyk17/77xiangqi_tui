use crate::{
    book::BookResponse,
    engine::{
        uci_ucci_engine::{
            info_state::uci_xiangqi_best_ready,
            ui_helpers::{move_human_from_fen, red_black_winrate_pct_from_wdl},
        },
        AnalysisSnapshot, EngineAnalysisStore, EngineAnalyzeResult,
    },
    game::BoardArrow,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct AnalysisService;

impl AnalysisService {
    pub fn idle_snapshot(&self) -> AnalysisSnapshot {
        AnalysisSnapshot::idle()
    }

    pub fn apply_book_response(&self, snapshot: &mut AnalysisSnapshot, response: &BookResponse) {
        snapshot.source = response.source.clone();
        snapshot.best_move = response
            .best_move
            .clone()
            .unwrap_or_else(|| "--".to_string());
        snapshot.score_text = response
            .best_winrate
            .map(|rate| format!("{rate:.1}%"))
            .unwrap_or_else(|| "--".to_string());
        snapshot.win_rate_text = response
            .best_winrate
            .map(|rate| format!("{rate:.1}%/--"))
            .unwrap_or_else(|| "--".to_string());
        snapshot.pv = response
            .candidates
            .iter()
            .filter_map(|c| c.move_uci.clone())
            .take(16)
            .collect();
    }

    pub fn apply_engine_store(
        &self,
        snapshot: &mut AnalysisSnapshot,
        store: &EngineAnalysisStore,
        query_mode: bool,
        pending_arrow: &mut Option<BoardArrow>,
    ) {
        self.apply_engine_result(snapshot, &store.result, &store.fen);
        self.sync_query_arrow(&store.result.best_move, query_mode, pending_arrow);
    }

    pub fn sync_query_arrow(
        &self,
        uci_best: &str,
        query_mode: bool,
        pending_arrow: &mut Option<BoardArrow>,
    ) {
        if query_mode {
            if let Some(arrow) = board_arrow_from_uci(uci_best) {
                *pending_arrow = Some(arrow);
            }
        }
    }

    pub fn apply_engine_result(
        &self,
        snapshot: &mut AnalysisSnapshot,
        result: &EngineAnalyzeResult,
        fen: &str,
    ) {
        snapshot.source = "engine".to_string();
        snapshot.depth = result.depth.unwrap_or(0).max(0) as u16;
        snapshot.nps = result.nps.unwrap_or(0);
        snapshot.nodes = result.nodes.unwrap_or(0);
        snapshot.time_text = format_time_ms(result.search_time_ms);
        snapshot.score_text = format_score(result);
        let uci = result.best_move.trim();
        snapshot.best_move = if uci_xiangqi_best_ready(uci) {
            move_human_from_fen(fen, uci)
        } else {
            uci.to_string()
        };
        let (red, black) = red_black_winrate_pct_from_wdl(fen, result.wdl);
        snapshot.win_rate_text = format_win_rate(red, black);
        snapshot.pv = if result.pv.is_empty() {
            result
                .candidates
                .first()
                .map(|c| c.pv.clone())
                .unwrap_or_default()
        } else {
            result.pv.clone()
        };
    }
}

fn format_time_ms(ms: Option<u64>) -> String {
    match ms {
        Some(v) if v >= 1000 => format!("{:.2}s", v as f64 / 1000.0),
        Some(v) => format!("{v}ms"),
        None => "--".to_string(),
    }
}

fn format_score(result: &EngineAnalyzeResult) -> String {
    if let Some(mate) = result.mate {
        return format!("M{mate}");
    }
    if let Some(cp) = result.score_cp {
        let pawns = cp as f64 / 100.0;
        if pawns > 0.0 {
            return format!("+{pawns:.2}");
        }
        return format!("{pawns:.2}");
    }
    if result.score.abs() < f64::EPSILON {
        return "0".to_string();
    }
    if result.score > 0.0 {
        format!("+{:.2}", result.score)
    } else {
        format!("{:.2}", result.score)
    }
}

fn format_win_rate(red: Option<f64>, black: Option<f64>) -> String {
    match (red, black) {
        (Some(r), Some(b)) => format!("{r:.1}%/{b:.1}%"),
        _ => "--/--".to_string(),
    }
}

fn board_arrow_from_uci(uci: &str) -> Option<BoardArrow> {
    if !uci_xiangqi_best_ready(uci) {
        return None;
    }
    crate::service::game::arrow_from_uci(uci)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::analysis_types::EngineAnalyzeResult;

    #[test]
    fn format_score_mate_and_cp() {
        let mate = EngineAnalyzeResult {
            mate: Some(3),
            ..EngineAnalyzeResult::default()
        };
        assert_eq!(format_score(&mate), "M3");

        let cp = EngineAnalyzeResult {
            score_cp: Some(45),
            ..EngineAnalyzeResult::default()
        };
        assert_eq!(format_score(&cp), "+0.45");
    }

    #[test]
    fn board_arrow_parses_valid_uci() {
        let arrow = board_arrow_from_uci("h2e2").expect("arrow");
        assert_eq!(arrow.from_file, 7);
        assert_eq!(arrow.from_rank, 7);
        assert!(board_arrow_from_uci("stub_move").is_none());
    }

    #[test]
    fn apply_engine_result_keeps_global_uci() {
        let svc = AnalysisService;
        let mut snap = AnalysisSnapshot::idle();
        let result = EngineAnalyzeResult {
            best_move: "h2e2".to_string(),
            depth: Some(18),
            pv: vec!["h2e2".to_string(), "h7e7".to_string()],
            ..EngineAnalyzeResult::default()
        };
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        svc.apply_engine_result(&mut snap, &result, fen);
        assert_eq!(snap.best_move, "h2e2");
        assert_eq!(snap.pv, vec!["h2e2", "h7e7"]);
    }
}
