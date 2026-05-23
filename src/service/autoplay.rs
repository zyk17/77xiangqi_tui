use std::time::{Duration, Instant};

use crate::{
    book::{BookConfig, BookResponse},
    engine::{EngineAnalyzeResult, uci_ucci_engine::info_state::uci_xiangqi_best_ready},
    game::GameState,
    service::{AnalysisService, BookService, book::BookQuery},
    xiangqi::Side,
};

pub const AI_MOVE_DELAY: Duration = Duration::from_millis(320);
pub const BOOK_ARROW_DELAY: Duration = Duration::from_millis(72);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AiPhase {
    #[default]
    Idle,
    WaitingToApply {
        uci: String,
        ready_at: Instant,
    },
}

pub fn book_config_usable(cfg: &BookConfig) -> bool {
    let local = cfg.local_enabled && !cfg.local_path.trim().is_empty();
    local || cfg.cloud_enabled
}

pub fn should_query_book_for_display(cfg: &BookConfig) -> bool {
    book_config_usable(cfg)
}

pub fn should_try_book_for_autoplay(game: &GameState, cfg: &BookConfig) -> bool {
    if !book_config_usable(cfg) {
        return false;
    }
    if game.query_mode || game.realtime_eval {
        return false;
    }
    if !game.history.at_head() {
        return false;
    }
    let max = cfg.max_halfmoves as usize;
    if max == 0 {
        return false;
    }
    game.history.halfmove_count() < max
}

pub fn ai_enabled_for_side(game: &GameState) -> bool {
    if game.is_game_over() || !game.history.at_head() {
        return false;
    }
    match game.side_to_move {
        Side::Red => game.red_ai,
        Side::Black => game.black_ai,
    }
}

pub fn book_has_preview(response: &BookResponse) -> bool {
    if response.candidates.is_empty() {
        return false;
    }
    let uci = response
        .best_move
        .as_deref()
        .or(response.move_uci.as_deref())
        .unwrap_or("");
    uci_xiangqi_best_ready(uci)
}

pub fn best_uci_from_book(response: &BookResponse) -> Option<String> {
    if !book_has_preview(response) {
        return None;
    }
    response
        .best_move
        .clone()
        .or_else(|| response.move_uci.clone())
        .filter(|uci| uci_xiangqi_best_ready(uci))
}

pub fn best_uci_from_engine(result: &EngineAnalyzeResult) -> Option<String> {
    let uci = result.best_move.trim();
    if uci_xiangqi_best_ready(uci) {
        Some(uci.to_string())
    } else {
        None
    }
}

pub struct AutoplayService;

impl AutoplayService {
    pub fn query_book_display(
        book: &BookService,
        analysis: &AnalysisService,
        game: &mut GameState,
        cfg: &BookConfig,
        fen: &str,
        set_query_arrow: bool,
    ) -> bool {
        if !should_query_book_for_display(cfg) {
            return false;
        }
        let response = book.query(
            BookQuery {
                fen,
                move_uci: None,
                ignore_play_opening_settings: false,
            },
            cfg,
        );
        if !book_has_preview(&response) {
            return false;
        }
        analysis.apply_book_response(&mut game.analysis, &response);
        if set_query_arrow {
            if let Some(uci) = best_uci_from_book(&response) {
                Self::set_pending_arrow(game, &uci);
            }
        }
        true
    }

    pub fn try_book_autoplay_move_for_game(
        book: &BookService,
        game: &GameState,
        cfg: &BookConfig,
        fen: &str,
    ) -> Option<String> {
        if !should_try_book_for_autoplay(game, cfg) {
            return None;
        }
        let response = book.query(
            BookQuery {
                fen,
                move_uci: None,
                ignore_play_opening_settings: false,
            },
            cfg,
        );
        best_uci_from_book(&response)
    }

    pub fn set_pending_arrow(game: &mut GameState, uci: &str) {
        if let Some(arrow) = crate::service::game::arrow_from_uci(uci) {
            game.pending_arrow = Some(arrow);
        }
    }

    pub fn begin_ai_wait(game: &mut GameState, uci: String, delay: Duration) -> AiPhase {
        Self::set_pending_arrow(game, &uci);
        AiPhase::WaitingToApply {
            uci,
            ready_at: Instant::now() + delay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::{BookCandidate, BookResponse};

    #[test]
    fn book_preview_requires_valid_uci() {
        let response = BookResponse {
            best_move: Some("h2e2".to_string()),
            candidates: vec![BookCandidate {
                move_uci: Some("h2e2".to_string()),
                ..BookCandidate::default()
            }],
            source: "obk".to_string(),
            ..BookResponse::default()
        };
        assert!(book_has_preview(&response));
        assert_eq!(best_uci_from_book(&response).as_deref(), Some("h2e2"));
    }

    #[test]
    fn autoplay_book_skipped_in_query_mode() {
        let game = GameState {
            query_mode: true,
            ..GameState::default()
        };
        let cfg = BookConfig {
            local_path: "x.obk".to_string(),
            local_enabled: true,
            cloud_enabled: false,
            pick_mode: "optimal".to_string(),
            max_halfmoves: 999,
        };
        assert!(!should_try_book_for_autoplay(&game, &cfg));
    }

    #[test]
    fn ai_enabled_follows_side_flags() {
        let mut game = GameState {
            red_ai: true,
            ..GameState::default()
        };
        assert!(ai_enabled_for_side(&game));
        game.side_to_move = Side::Black;
        assert!(!ai_enabled_for_side(&game));
    }
}
