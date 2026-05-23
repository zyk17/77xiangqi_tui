use crate::engine::AnalysisSnapshot;

#[derive(Debug, Default, Clone, Copy)]
pub struct EngineService;

impl EngineService {
    pub fn idle_snapshot(&self) -> AnalysisSnapshot {
        AnalysisSnapshot::idle()
    }
}
