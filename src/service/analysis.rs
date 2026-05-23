use crate::{book::BookResponse, engine::AnalysisSnapshot};

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
            .filter_map(|candidate| candidate.move_uci.clone())
            .take(16)
            .collect();
    }
}
