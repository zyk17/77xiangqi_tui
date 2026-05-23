//! 集成测用：短路真实子进程 `analyze`。

use super::types::EngineAnalyzeRequest;
use super::UciUcciEngine;
use crate::engine::analysis_types::EngineAnalyzeResult;
use std::sync::{Arc, Mutex};

type TestAnalyzeHook =
    Arc<dyn for<'a> Fn(EngineAnalyzeRequest<'a>) -> Option<EngineAnalyzeResult> + Send + Sync>;

static TEST_ANALYZE_HOOK: Mutex<Option<TestAnalyzeHook>> = Mutex::new(None);

pub(crate) fn try_test_analyze_hook(req: &EngineAnalyzeRequest<'_>) -> Option<EngineAnalyzeResult> {
    let guard = TEST_ANALYZE_HOOK.lock().ok()?;
    let hook = guard.as_ref()?;
    hook(EngineAnalyzeRequest {
        fen: req.fen,
        depth: req.depth,
        movetime_ms: req.movetime_ms,
        search_moves: req.search_moves,
        search_nodes: req.search_nodes,
        multipv_override: req.multipv_override,
        cancel: None,
    })
}

impl UciUcciEngine {
    pub fn set_test_analyze_hook(hook: Option<TestAnalyzeHook>) {
        if let Ok(mut g) = TEST_ANALYZE_HOOK.lock() {
            *g = hook;
        }
    }

    pub fn clear_test_analyze_hook() {
        Self::set_test_analyze_hook(None);
    }
}
