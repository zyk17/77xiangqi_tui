use std::sync::{Arc, Mutex};

use crate::engine::{
    EngineAnalysisStore, EngineAnalyzeResult, EngineConfig, EngineStreamRuntime,
    uci_ucci_engine::{EngineConfigureRequest, UciUcciEngine},
};
use crate::runtime_log;

/// 引擎调度：单进程 `UciUcciEngine`，流式 infinite 与 AI `go movetime` 互斥（对齐 GUI）。
pub struct EngineService {
    engine: Arc<Mutex<UciUcciEngine>>,
    stream: EngineStreamRuntime,
}

impl Default for EngineService {
    fn default() -> Self {
        let engine = Arc::new(Mutex::new(UciUcciEngine::new(None)));
        Self {
            stream: EngineStreamRuntime::new(engine.clone()),
            engine,
        }
    }
}

impl std::fmt::Debug for EngineService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineService")
            .field("streaming", &self.stream.is_running())
            .finish()
    }
}

impl EngineService {
    pub fn ensure_stream(&self, fen: &str, cfg: &EngineConfig, want_stream: bool) {
        self.stream.ensure_stream(fen, cfg, want_stream);
    }

    pub fn stop_stream(&self) {
        self.stream.stop_stream();
    }

    /// AI 自动走子：先停 infinite，再在同进程上 `analyze_autoplay_once`（主线程同步）。
    pub fn run_autoplay_once(&self, fen: &str, cfg: &EngineConfig) -> EngineAnalyzeResult {
        self.stream.stop_stream();
        let fen = fen.trim();
        let path = cfg.path.trim();
        if path.is_empty() || fen.is_empty() {
            runtime_log::warn("[autoplay] skip empty path or fen");
            return EngineAnalyzeResult::default();
        }
        let Ok(mut eng) = self.engine.lock() else {
            runtime_log::error("[autoplay] engine mutex poisoned");
            return EngineAnalyzeResult::default();
        };
        self.apply_config(&mut eng, cfg);
        runtime_log::info(format!("[autoplay] analyze start fen={fen}"));
        let result = eng.analyze_autoplay_once(fen);
        runtime_log::info(format!(
            "[autoplay] analyze done best_move={} depth={:?}",
            result.best_move, result.depth
        ));
        result
    }

    pub fn current_store(&self) -> EngineAnalysisStore {
        self.stream.clone_store()
    }

    pub fn snapshot_if_newer(&self, last_revision: u64) -> Option<(EngineAnalysisStore, u64)> {
        let revision = self.stream.store_revision();
        if revision == last_revision {
            return None;
        }
        Some((self.stream.clone_store(), revision))
    }

    pub fn is_streaming(&self) -> bool {
        self.stream.is_running()
    }

    fn apply_config(&self, eng: &mut UciUcciEngine, cfg: &EngineConfig) {
        let path = cfg.path.trim();
        if path.is_empty() {
            return;
        }
        eng.configure(EngineConfigureRequest {
            engine_path: Some(path.to_string()),
            threads: Some(i32::from(cfg.threads)),
            hash_mb: Some(cfg.hash_mb as i32),
            repetition_rule: Some(cfg.variant.clone()),
            draw_rule: Some(cfg.rule.clone()),
            skill_level: Some(i32::from(cfg.skill_level)),
            engine_protocol_preference: Some(cfg.protocol.preference().to_string()),
            engine_config_path: None,
            protocol_detected_for_path: None,
            protocol_detected: None,
        });
    }
}
