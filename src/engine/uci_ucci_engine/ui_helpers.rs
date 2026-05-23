use crate::engine::analysis_types::EngineAnalyzeResult;

pub fn move_human_from_fen(fen: &str, uci: &str) -> String {
    let _ = fen;
    uci.to_string()
}

/// 由 WDL 推算红/黑胜率百分比；无法计算时返回 `(None, None)`。
pub fn red_black_winrate_pct_from_wdl(
    fen: &str,
    wdl: Option<[u64; 3]>,
) -> (Option<f64>, Option<f64>) {
    let Some([w, d, l]) = wdl else {
        return (None, None);
    };
    let w = w as f64;
    let d = d as f64;
    let l = l as f64;
    let total = w + d + l;
    if total <= 0.0 {
        return (None, None);
    }
    let stm_win = (w + d * 0.5) * 100.0 / total;
    let opp_win = 100.0 - stm_win;
    if side_to_move_is_red(fen) {
        (Some(stm_win), Some(opp_win))
    } else {
        (Some(opp_win), Some(stm_win))
    }
}

pub(crate) fn stub_result() -> EngineAnalyzeResult {
    EngineAnalyzeResult::stub()
}

fn side_to_move_is_red(fen: &str) -> bool {
    let side = fen.split_whitespace().nth(1).unwrap_or("r");
    matches!(side, "w" | "W" | "r" | "R")
}
