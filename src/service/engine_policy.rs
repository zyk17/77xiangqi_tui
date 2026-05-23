//! 是否挂 `go infinite` 共享流（对齐 GUI `engineStreamPolicy.shouldAttachInfiniteStreamPlay`）。

use crate::game::GameState;

/// 局面分析（查询）或实时评估开启时，才可能挂 infinite 流。
pub fn wants_shared_infinite_stream(game: &GameState) -> bool {
    let analysis = game.query_mode;
    let auto_eval = game.realtime_eval;
    if !analysis && !auto_eval {
        return false;
    }
    let both_ai = game.red_ai && game.black_ai;
    if auto_eval && both_ai && !analysis {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;

    #[test]
    fn infinite_off_when_no_eval_modes() {
        let game = GameState::default();
        assert!(!wants_shared_infinite_stream(&game));
    }

    #[test]
    fn infinite_off_for_dual_ai_realtime_only() {
        let game = GameState {
            red_ai: true,
            black_ai: true,
            realtime_eval: true,
            query_mode: false,
            ..GameState::default()
        };
        assert!(!wants_shared_infinite_stream(&game));
    }

    #[test]
    fn infinite_on_for_query_mode() {
        let game = GameState {
            query_mode: true,
            ..GameState::default()
        };
        assert!(wants_shared_infinite_stream(&game));
    }
}
