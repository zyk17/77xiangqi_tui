//! UCI 风格 `info` 行共用解析。中国象棋引擎在 **UCCI 模式** 下搜索阶段也常输出同类 `info` 文本。

use crate::engine::analysis_types::EngineInfoCandidate;

fn parse_i32_token_loose(token: &str) -> Option<i32> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    let t = t.trim_end_matches([',', ';']);
    t.parse::<i32>().ok()
}

/// UCI `info` 行中键后紧跟的 u64（如 `time` 毫秒、`nps`）。
pub(crate) fn uci_info_u64_after(parts: &[&str], key: &str) -> Option<u64> {
    let i = parts.iter().position(|&x| x == key)?;
    parts.get(i + 1)?.parse().ok()
}

/// 单条 `info ...` 行解析结果（已按词切分）。
pub(crate) struct ParsedUciStyleInfo {
    pub multipv: i32,
    pub cand_score: f64,
    /// UCI `score cp` 原始厘兵（未 `/100`），供复盘批处理；无 `cp` 分时为 `None`。
    pub cp_centipawns: Option<i32>,
    pub has_score: bool,
    pub mate: Option<i32>,
    pub pv_tok: Vec<String>,
    pub depth: Option<i32>,
    pub search_time_ms: Option<u64>,
    pub nps: Option<u64>,
    pub nodes: Option<u64>,
    pub wdl: Option<[u64; 3]>,
}

/// 解析 `parts`（首词须为 `info`）。
pub(crate) fn parse_uci_style_info_tokens(parts: &[&str]) -> Option<ParsedUciStyleInfo> {
    if parts.first().copied() != Some("info") {
        return None;
    }
    let search_time_ms = uci_info_u64_after(parts, "time");
    let nps = uci_info_u64_after(parts, "nps");
    let nodes = uci_info_u64_after(parts, "nodes");
    let mut multipv = 1i32;
    if let Some(i) = parts.iter().position(|&x| x == "multipv")
        && i + 1 < parts.len()
    {
        multipv = parts[i + 1].parse().unwrap_or(1);
    }
    let mut cand_score = 0.0f64;
    let mut cp_centipawns: Option<i32> = None;
    let mut has_score = false;
    let mut mate = None;
    if let Some(i) = parts.iter().position(|&x| x == "score")
        && i + 1 < parts.len()
    {
        match parts[i + 1] {
            "cp" | "cp," => {
                if i + 2 < parts.len()
                    && let Some(cp_i) = parse_i32_token_loose(parts[i + 2])
                {
                    cp_centipawns = Some(cp_i);
                    cand_score = f64::from(cp_i) / 100.0;
                    has_score = true;
                }
            }
            "mate" | "mate," => {
                if i + 2 < parts.len()
                    && let Some(mate_in) = parse_i32_token_loose(parts[i + 2])
                {
                    mate = Some(mate_in);
                    cand_score = if mate_in > 0 {
                        9999.0
                    } else if mate_in < 0 {
                        -9999.0
                    } else {
                        0.0
                    };
                    has_score = true;
                }
            }
            raw => {
                if let Some(cp_i) = parse_i32_token_loose(raw) {
                    cp_centipawns = Some(cp_i);
                    cand_score = f64::from(cp_i) / 100.0;
                    has_score = true;
                }
            }
        }
    }
    let mut pv_tok: Vec<String> = vec![];
    if let Some(i) = parts.iter().position(|&x| x == "pv") {
        pv_tok = parts[i + 1..].iter().map(|s| s.to_string()).collect();
    }
    let mut depth = None;
    if let Some(i) = parts.iter().position(|&x| x == "depth")
        && i + 1 < parts.len()
    {
        depth = parts[i + 1].parse().ok();
    }
    let wdl = if let Some(i) = parts.iter().position(|&x| x == "wdl") {
        let w = parts.get(i + 1).and_then(|x| x.parse::<u64>().ok());
        let d = parts.get(i + 2).and_then(|x| x.parse::<u64>().ok());
        let l = parts.get(i + 3).and_then(|x| x.parse::<u64>().ok());
        match (w, d, l) {
            (Some(w), Some(d), Some(l)) => Some([w, d, l]),
            _ => None,
        }
    } else {
        None
    };
    Some(ParsedUciStyleInfo {
        multipv,
        cand_score,
        cp_centipawns,
        has_score,
        mate,
        pv_tok,
        depth,
        search_time_ms,
        nps,
        nodes,
        wdl,
    })
}

pub(crate) fn candidate_from_parsed(
    parsed: &ParsedUciStyleInfo,
    best_move_fallback: &str,
    previous: Option<&EngineInfoCandidate>,
) -> EngineInfoCandidate {
    let prev_move = previous
        .map(|c| c.best_move.as_str())
        .unwrap_or(best_move_fallback);
    let move0 = parsed
        .pv_tok
        .first()
        .cloned()
        .unwrap_or_else(|| prev_move.to_string());
    let prev_score = previous.map(|c| c.score).unwrap_or(0.0);
    let score = if parsed.has_score {
        parsed.cand_score
    } else {
        prev_score
    };
    let prev_pv = previous.map(|c| c.pv.clone()).unwrap_or_default();
    let pv = if parsed.pv_tok.is_empty() {
        prev_pv
    } else {
        parsed.pv_tok.clone()
    };
    let prev_depth = previous.and_then(|c| c.depth);
    let depth = parsed.depth.or(prev_depth);
    let prev_nodes = previous.and_then(|c| c.nodes);
    let nodes = parsed.nodes.or(prev_nodes);
    let prev_wdl = previous.and_then(|c| c.wdl);
    let wdl = parsed.wdl.or(prev_wdl);
    let prev_mate = previous.and_then(|c| c.mate);
    let mate = if parsed.has_score {
        parsed.mate
    } else {
        parsed.mate.or(prev_mate)
    };
    EngineInfoCandidate {
        rank: parsed.multipv,
        best_move: move0,
        score,
        score_cp: parsed.cp_centipawns,
        mate,
        pv,
        depth,
        nodes,
        wdl,
    }
}

#[cfg(test)]
mod tests {
    use super::{EngineInfoCandidate, candidate_from_parsed, parse_uci_style_info_tokens};

    #[test]
    fn parse_score_and_wdl_with_lowerbound() {
        let line = "info depth 22 seldepth 34 multipv 1 score cp 6 lowerbound wdl 23 973 4 nodes 2024471 nps 1106873 hashfull 674 tbhits 0 time 1829 pv h2e2";
        let parts: Vec<&str> = line.split_whitespace().collect();
        let parsed = parse_uci_style_info_tokens(&parts).expect("should parse info line");
        assert!(parsed.has_score);
        assert!((parsed.cand_score - 0.06).abs() < 1e-9);
        assert_eq!(parsed.cp_centipawns, Some(6));
        assert_eq!(parsed.mate, None);
        assert_eq!(parsed.wdl, Some([23, 973, 4]));
        assert_eq!(parsed.pv_tok, vec!["h2e2".to_string()]);
    }

    #[test]
    fn candidate_merge_keeps_previous_when_new_line_has_no_score_or_pv() {
        let info = parse_uci_style_info_tokens(&["info", "depth", "18"]).expect("parse");
        let prev = EngineInfoCandidate {
            rank: 1,
            best_move: "h2e2".to_string(),
            score: 0.06,
            mate: None,
            pv: vec!["h2e2".to_string()],
            depth: Some(22),
            nodes: Some(1000),
            wdl: Some([23, 973, 4]),
            ..EngineInfoCandidate::default()
        };
        let merged = candidate_from_parsed(&info, "stub_move", Some(&prev));
        assert_eq!(merged.best_move, "h2e2");
        assert!((merged.score - 0.06).abs() < 1e-9);
        assert_eq!(merged.pv, vec!["h2e2".to_string()]);
        assert!(merged.mate.is_none());
    }

    #[test]
    fn candidate_merge_clears_old_mate_when_new_cp_score_arrives() {
        let info = parse_uci_style_info_tokens(&["info", "depth", "19", "score", "cp", "12"])
            .expect("parse");
        let prev = EngineInfoCandidate {
            rank: 1,
            best_move: "h2e2".to_string(),
            score: 9999.0,
            mate: Some(5),
            pv: vec!["h2e2".to_string()],
            depth: Some(22),
            nodes: Some(1000),
            wdl: Some([23, 973, 4]),
            ..EngineInfoCandidate::default()
        };
        let merged = candidate_from_parsed(&info, "stub_move", Some(&prev));
        assert!((merged.score - 0.12).abs() < 1e-9);
        assert!(merged.mate.is_none());
    }

    #[test]
    fn parse_mate_score_keeps_non_zero_score_signal() {
        let line = "info depth 20 multipv 1 score mate 3 wdl 1000 0 0 nodes 12345 pv h2e2";
        let parts: Vec<&str> = line.split_whitespace().collect();
        let parsed = parse_uci_style_info_tokens(&parts).expect("should parse mate info");
        assert!(parsed.has_score);
        assert_eq!(parsed.cand_score, 9999.0);
        assert_eq!(parsed.mate, Some(3));
        assert_eq!(parsed.wdl, Some([1000, 0, 0]));
    }

    #[test]
    fn parse_spin_storm_style_score_without_cp_keyword() {
        let line = "info depth 12 score 156 time 1240 nodes 890234 nps 717931 pv h2i2 h7g7";
        let parts: Vec<&str> = line.split_whitespace().collect();
        let parsed =
            parse_uci_style_info_tokens(&parts).expect("should parse spin-storm style info");
        assert!(parsed.has_score);
        assert_eq!(parsed.cp_centipawns, Some(156));
        assert!((parsed.cand_score - 1.56).abs() < 1e-9);
        assert_eq!(parsed.depth, Some(12));
        assert_eq!(parsed.search_time_ms, Some(1240));
        assert_eq!(parsed.nodes, Some(890_234));
        assert_eq!(parsed.nps, Some(717_931));
        assert_eq!(parsed.pv_tok, vec!["h2i2".to_string(), "h7g7".to_string()]);
    }
}
