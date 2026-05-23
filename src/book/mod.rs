//! 开局库层：TUI 版统一入口。
//! - `obk`: 本地 `.obk/.db`
//! - `xqb`: 本地 `.xqb`
//! - `chessdb`: 远程云库

mod chessdb;
pub mod obk;
mod xqb;
mod zobrist_openbook;

use std::path::Path;

use crate::runtime_log;

#[derive(Debug, Clone)]
pub struct BookConfig {
    pub local_path: String,
    pub local_enabled: bool,
    pub cloud_enabled: bool,
    pub pick_mode: String,
    pub max_halfmoves: u16,
}

impl Default for BookConfig {
    fn default() -> Self {
        Self {
            local_path: String::new(),
            local_enabled: true,
            cloud_enabled: false,
            pick_mode: "positive_random".to_string(),
            max_halfmoves: 999,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BookCandidate {
    pub move_uci: Option<String>,
    pub rank: Option<i64>,
    pub score: Option<f64>,
    pub winrate: Option<f64>,
    pub winrate_raw: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct BookResponse {
    pub move_uci: Option<String>,
    pub best_move: Option<String>,
    pub best_winrate: Option<f64>,
    pub move_eval: Option<BookCandidate>,
    pub candidates: Vec<BookCandidate>,
    pub source: String,
    pub error: Option<String>,
}

impl BookResponse {
    pub fn empty(source: &str) -> Self {
        Self {
            source: source.to_string(),
            ..Self::default()
        }
    }

    pub(crate) fn with_move_eval(
        candidates: Vec<BookCandidate>,
        move_uci: Option<String>,
        source: &str,
    ) -> Self {
        let move_key: String = move_uci
            .as_deref()
            .unwrap_or("")
            .trim()
            .chars()
            .take(4)
            .collect();
        let best_move = candidates.first().and_then(|c| c.move_uci.clone());
        let best_winrate = candidates.first().and_then(|c| c.winrate);

        if move_key.is_empty() {
            return Self {
                move_uci: best_move.clone(),
                best_move,
                best_winrate,
                move_eval: None,
                candidates,
                source: source.to_string(),
                error: None,
            };
        }

        let move_eval = candidates
            .iter()
            .find(|candidate| {
                candidate
                    .move_uci
                    .as_deref()
                    .is_some_and(|mv| mv == move_key.as_str())
            })
            .cloned();

        Self {
            move_uci: Some(move_key),
            best_move,
            best_winrate,
            move_eval,
            candidates,
            source: source.to_string(),
            error: None,
        }
    }
}

pub fn query_opening_book(
    fen: &str,
    move_uci: Option<String>,
    cfg: &BookConfig,
    ignore_play_opening_settings: bool,
) -> BookResponse {
    let for_eval = move_uci.is_some();
    let local_path = cfg.local_path.trim();
    let path_nonempty = !local_path.is_empty();

    if ignore_play_opening_settings {
        if path_nonempty {
            let local = query_local_book_unified(local_path, fen, move_uci.clone());
            if !local.candidates.is_empty() {
                return apply_opening_book_pick_mode(local, cfg, for_eval);
            }
        }
        return apply_opening_book_pick_mode(
            ensure_unified_opening_book_shape(
                chessdb::query_opening_book(fen, move_uci),
                "chessdb",
            ),
            cfg,
            for_eval,
        );
    }

    let want_local = path_nonempty && cfg.local_enabled;
    let want_cloud = cfg.cloud_enabled;

    runtime_log::debug(format!(
        "[book] local={} cloud={} path={} eval={}",
        want_local, want_cloud, local_path, for_eval
    ));

    if want_local {
        return apply_opening_book_pick_mode(
            query_local_book_unified(local_path, fen, move_uci),
            cfg,
            for_eval,
        );
    }
    if want_cloud {
        return apply_opening_book_pick_mode(
            ensure_unified_opening_book_shape(
                chessdb::query_opening_book(fen, move_uci),
                "chessdb",
            ),
            cfg,
            for_eval,
        );
    }
    BookResponse::empty("none")
}

fn ensure_unified_opening_book_shape(mut response: BookResponse, source: &str) -> BookResponse {
    response.source = source.to_string();
    if response.best_move.is_none() {
        response.best_move = response.move_uci.clone();
    }
    response
}

fn apply_opening_book_pick_mode(
    mut response: BookResponse,
    cfg: &BookConfig,
    for_move_eval: bool,
) -> BookResponse {
    if for_move_eval || response.candidates.is_empty() {
        return response;
    }
    if cfg.pick_mode != "positive_random" {
        return response;
    }

    fn candidate_positive(candidate: &BookCandidate) -> bool {
        if candidate.score.is_some_and(|score| score > 0.0) {
            return true;
        }
        if let Some(winrate) = candidate.winrate {
            if winrate > 50.0 {
                return true;
            }
            if winrate > 0.5 && winrate <= 1.0 {
                return true;
            }
        }
        false
    }

    let positive: Vec<usize> = response
        .candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate_positive(candidate))
        .map(|(index, _)| index)
        .collect();
    let pool: Vec<usize> = if positive.is_empty() {
        (0..response.candidates.len()).collect()
    } else {
        positive
    };
    let pick = pool[pick_pool_index(pool.len())];

    if let Some(chosen) = response.candidates.get(pick).cloned() {
        response.move_uci = chosen.move_uci.clone();
        response.best_move = chosen.move_uci.clone();
        response.best_winrate = chosen.winrate;
    }
    response
}

fn pick_pool_index(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let mut buffer = [0_u8; 8];
    if getrandom::fill(&mut buffer).is_ok() {
        (u64::from_ne_bytes(buffer) % len as u64) as usize
    } else {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(12_345);
        (nanos % len as u64) as usize
    }
}

fn query_local_book(path: &str, fen: &str, move_uci: Option<String>) -> BookResponse {
    let ext = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "xqb" => xqb::query_local(fen, move_uci, path),
        _ => obk::query_local(fen, move_uci, path),
    }
}

fn query_local_book_unified(path: &str, fen: &str, move_uci: Option<String>) -> BookResponse {
    let local = query_local_book(path, fen, move_uci);
    let source = local.source.clone();
    ensure_unified_opening_book_shape(local, &source)
}
