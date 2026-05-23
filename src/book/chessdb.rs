//! chessdb.cn 远程开局查询（当前唯一实现）。

use crate::book::{BookCandidate, BookResponse};
use regex::Regex;
use std::sync::LazyLock;
use urlencoding::encode;

static FLOAT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[-+]?\d+(?:\.\d+)?").unwrap());

fn extract_float(val: Option<&str>) -> Option<f64> {
    let s = val?.trim();
    if s.is_empty() || matches!(s.to_lowercase().as_str(), "?" | "??" | "none" | "null") {
        return None;
    }
    FLOAT_RE.find(s).and_then(|m| m.as_str().parse().ok())
}

fn parse_candidates(resp_text: &str) -> Vec<BookCandidate> {
    let mut candidates: Vec<BookCandidate> = Vec::new();
    if resp_text.is_empty() {
        return candidates;
    }
    for part in resp_text.split('|') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let mut kv: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for f in part.split(',') {
            let f = f.trim();
            if let Some((k, v)) = f.split_once(':') {
                kv.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }
        let mv = kv
            .get("move")
            .or_else(|| kv.get("egtb"))
            .or_else(|| kv.get("search"));
        let Some(mv) = mv else { continue };
        let mv_uci: String = mv.chars().take(4).collect();
        if mv_uci.len() < 4 {
            continue;
        }
        let rank = kv.get("rank").and_then(|s| s.parse::<i64>().ok());
        let score = kv
            .get("score")
            .and_then(|s| extract_float(Some(s.as_str())));
        let mut winrate_raw: Option<&String> = None;
        for k in kv.keys() {
            if k.contains("winrate") || k.contains("win_rate") {
                winrate_raw = kv.get(k);
                break;
            }
        }
        let winrate = winrate_raw.and_then(|s| extract_float(Some(s.as_str())));
        candidates.push(BookCandidate {
            move_uci: Some(mv_uci),
            rank,
            score,
            winrate,
            winrate_raw: winrate_raw.map(|s| s.to_string()),
        });
    }
    candidates
}

fn bad_response(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("invalid board") || t.contains("unknown") || t.contains("nobestmove")
}

/// 与 `CoreApi.query_opening_book` 行为对齐（chessdb.cn）。
pub fn query_opening_book(fen: &str, move_uci: Option<String>) -> BookResponse {
    let move_uci = move_uci.as_deref().unwrap_or("").trim();
    let move_key: String = move_uci.chars().take(4).collect();

    let fetch = |url: &str| -> String {
        ureq::get(url)
            .set("User-Agent", "77xiangqi/1.0")
            .call()
            .ok()
            .and_then(|r| r.into_string().ok())
            .unwrap_or_default()
            .trim()
            .to_string()
    };

    let result: BookResponse = (|| {
        let candidates: Vec<BookCandidate> = if !move_key.is_empty() {
            let url = format!(
                "http://www.chessdb.cn/chessdb.php?action=queryall&board={}&showall=1&learn=0",
                encode(fen)
            );
            let text = fetch(&url);
            if text.is_empty() || bad_response(&text) {
                return BookResponse {
                    move_uci: if move_key.len() == 4 {
                        Some(move_key.clone())
                    } else {
                        None
                    },
                    best_move: None,
                    best_winrate: None,
                    move_eval: None,
                    candidates: vec![],
                    source: "chessdb".to_string(),
                    error: None,
                };
            }
            parse_candidates(&text)
        } else {
            let url = format!(
                "http://www.chessdb.cn/chessdb.php?action=querybest&board={}",
                encode(fen)
            );
            let text = fetch(&url);
            if text.is_empty() || bad_response(&text) {
                return BookResponse {
                    move_uci: None,
                    best_move: None,
                    best_winrate: None,
                    move_eval: None,
                    candidates: vec![],
                    source: "chessdb".to_string(),
                    error: None,
                };
            }
            let mut c = parse_candidates(&text);
            if c.len() < 5 {
                let url_all = format!(
                    "http://www.chessdb.cn/chessdb.php?action=queryall&board={}&showall=1&learn=0",
                    encode(fen)
                );
                let text2 = fetch(&url_all);
                if !text2.is_empty() && !bad_response(&text2) {
                    c = parse_candidates(&text2);
                }
            }
            c
        };

        if candidates.is_empty() {
            return BookResponse {
                move_uci: if move_key.is_empty() {
                    None
                } else if move_key.len() == 4 {
                    Some(move_key.clone())
                } else {
                    None
                },
                best_move: None,
                best_winrate: None,
                move_eval: None,
                candidates: vec![],
                source: "chessdb".to_string(),
                error: None,
            };
        }

        let winrate_candidates: Vec<&BookCandidate> =
            candidates.iter().filter(|c| c.winrate.is_some()).collect();
        let best_winrate = winrate_candidates
            .iter()
            .max_by(|a, b| {
                let fa = a.winrate.unwrap_or(0.0);
                let fb = b.winrate.unwrap_or(0.0);
                fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|c| c.winrate);

        let valid: Vec<BookCandidate> = candidates
            .iter()
            .filter(|c| c.winrate.is_some())
            .cloned()
            .collect();

        let mut top: Vec<BookCandidate> = if !valid.is_empty() {
            let all_ranked = valid.iter().all(|c| c.rank.map(|r| r > 0).unwrap_or(false));
            let mut vs = valid;
            if all_ranked {
                vs.sort_by_key(|c| c.rank.unwrap_or(0));
            } else {
                vs.sort_by(|a, b| {
                    let fa = a.winrate.unwrap_or(0.0);
                    let fb = b.winrate.unwrap_or(0.0);
                    fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            vs.into_iter().take(5).collect()
        } else {
            let loose: Vec<BookCandidate> = candidates
                .iter()
                .filter(|c| c.move_uci.as_deref().is_some_and(|s| s.len() >= 4))
                .cloned()
                .collect();
            let all_ranked =
                !loose.is_empty() && loose.iter().all(|c| c.rank.map(|r| r > 0).unwrap_or(false));
            if all_ranked {
                let mut l = loose;
                l.sort_by_key(|c| c.rank.unwrap_or(0));
                l.into_iter().take(5).collect()
            } else {
                let mut l = loose;
                l.sort_by(|a, b| {
                    let sa = a.score.unwrap_or(0.0);
                    let sb = b.score.unwrap_or(0.0);
                    sb.partial_cmp(&sa)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| {
                            let ma = a.move_uci.as_deref().unwrap_or("");
                            let mb = b.move_uci.as_deref().unwrap_or("");
                            ma.cmp(mb)
                        })
                });
                l.into_iter().take(5).collect()
            }
        };

        for (i, c) in top.iter_mut().enumerate() {
            c.rank = Some(i as i64 + 1);
        }

        let best_move = top.first().and_then(|c| c.move_uci.clone());

        if move_key.is_empty() {
            return BookResponse {
                move_uci: best_move.clone(),
                best_move,
                best_winrate,
                move_eval: None,
                candidates: top,
                source: "chessdb".to_string(),
                error: None,
            };
        }

        let move_eval = candidates
            .iter()
            .find(|c| {
                c.move_uci
                    .as_deref()
                    .is_some_and(|m| m == move_key.as_str())
            })
            .cloned();

        BookResponse {
            move_uci: Some(move_key),
            best_move,
            best_winrate,
            move_eval,
            candidates: top,
            source: "chessdb".to_string(),
            error: None,
        }
    })();

    result
}
