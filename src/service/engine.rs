use crate::engine::{EngineAnalysisStore, EngineConfig, EngineStreamRuntime};

#[derive(Default)]
pub struct EngineService {
    stream: EngineStreamRuntime,
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
}
