use crate::engine::{EngineAnalysisStore, EngineAnalyzeResult, EngineConfig, EngineStreamRuntime};

/// 引擎调度：单进程 `UciUcciEngine`，流式 infinite 与 AI `go movetime` 互斥（对齐 GUI）。
pub struct EngineService {
    stream: EngineStreamRuntime,
}

impl Default for EngineService {
    fn default() -> Self {
        use std::sync::{Arc, Mutex};

        use crate::engine::uci_ucci_engine::UciUcciEngine;

        let engine = Arc::new(Mutex::new(UciUcciEngine::new(None)));
        Self {
            stream: EngineStreamRuntime::new(engine),
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

    /// 仅停 `go infinite`（不关 AI 后台 `go`）。
    pub fn stop_stream(&self) {
        self.stream.stop_infinite_stream();
    }

    /// 停 infinite 并等待 AI `go`（新局、`/stop`、退出）。
    pub fn stop_all(&self) {
        self.stream.stop_all();
    }

    /// 无任何模式需要引擎时，终止子进程（对齐 GUI `clear_engine_mode_state`）。
    pub fn release_if_idle(&self) {
        if self.stream.needs_process_release() {
            self.stream.release_engine_process();
        }
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

    pub fn is_autoplay_running(&self) -> bool {
        self.stream.is_autoplay_running()
    }

    /// 后台 `go`，思考过程中可通过 `current_store` / `snapshot_if_newer` 刷新箭头与 D 区。
    pub fn spawn_autoplay_once(&self, fen: &str, cfg: &EngineConfig) {
        self.stream.spawn_autoplay_once(fen, cfg);
    }

    pub fn poll_autoplay_done(&self) -> Option<EngineAnalyzeResult> {
        self.stream.poll_autoplay_done()
    }
}
