//! TUI 流式分析：后台 `go infinite`，主线程每帧读取 [`EngineAnalysisStore`]。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use crate::engine::EngineConfig;
use crate::engine::analysis_store::EngineAnalysisStore;
use crate::engine::uci_ucci_engine::{EngineConfigureRequest, UciUcciEngine};

fn lock_mutex<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct EngineStreamRuntime {
    engine: Arc<Mutex<UciUcciEngine>>,
    store: Arc<Mutex<EngineAnalysisStore>>,
    stop: Arc<AtomicBool>,
    join: Mutex<Option<JoinHandle<()>>>,
    session_gen: Arc<AtomicU64>,
    active_fen: Mutex<String>,
}

impl Default for EngineStreamRuntime {
    fn default() -> Self {
        Self::new(Arc::new(Mutex::new(UciUcciEngine::new(None))))
    }
}

impl EngineStreamRuntime {
    pub fn new(engine: Arc<Mutex<UciUcciEngine>>) -> Self {
        Self {
            engine,
            store: Arc::new(Mutex::new(EngineAnalysisStore::empty_for_fen(""))),
            stop: Arc::new(AtomicBool::new(false)),
            join: Mutex::new(None),
            session_gen: Arc::new(AtomicU64::new(0)),
            active_fen: Mutex::new(String::new()),
        }
    }

    pub fn store_revision(&self) -> u64 {
        lock_mutex(&self.store).revision
    }

    pub fn clone_store(&self) -> EngineAnalysisStore {
        lock_mutex(&self.store).clone()
    }

    pub fn configure_engine(&self, cfg: &EngineConfig) {
        let path = cfg.path.trim();
        if path.is_empty() {
            return;
        }
        if let Ok(mut eng) = self.engine.lock() {
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

    pub fn is_running(&self) -> bool {
        lock_mutex(&self.join).is_some()
    }

    pub fn active_fen(&self) -> String {
        lock_mutex(&self.active_fen).clone()
    }

    /// 启动或重启流式分析（局面或配置变化时调用）。
    pub fn ensure_stream(&self, fen: &str, cfg: &EngineConfig, want_stream: bool) {
        if !want_stream || cfg.path.trim().is_empty() {
            self.stop_stream();
            return;
        }
        let fen = fen.trim().to_string();
        if self.is_running() && self.active_fen() == fen {
            return;
        }
        self.stop_stream();
        self.configure_engine(cfg);
        let session = self.session_gen.fetch_add(1, Ordering::SeqCst) + 1;
        *lock_mutex(&self.active_fen) = fen.clone();
        {
            let mut guard = lock_mutex(&self.store);
            guard.reset_for_stream(&fen);
        }
        self.stop.store(false, Ordering::SeqCst);
        let eng = self.engine.clone();
        let store = self.store.clone();
        let stop = self.stop.clone();
        let live_session = self.session_gen.clone();
        let multipv = i32::from(cfg.multi_pv.max(1));
        let handle = thread::spawn(move || {
            if let Ok(mut engine) = eng.lock() {
                engine.run_infinite_analysis(&fen, &store, &stop, &live_session, session, multipv);
            }
        });
        *lock_mutex(&self.join) = Some(handle);
    }

    pub fn stop_stream(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = lock_mutex(&self.join).take() {
            let _ = handle.join();
        }
        self.stop.store(false, Ordering::SeqCst);
        *lock_mutex(&self.active_fen) = String::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineProtocol;

    fn sample_config(path: &str) -> EngineConfig {
        EngineConfig {
            path: path.to_string(),
            protocol: EngineProtocol::Uci,
            threads: 2,
            hash_mb: 128,
            skill_level: 20,
            multi_pv: 1,
            variant: "AsianRule".to_string(),
            rule: "None".to_string(),
        }
    }

    #[test]
    fn ensure_stream_noop_when_disabled() {
        let rt = EngineStreamRuntime::default();
        let cfg = sample_config("C:\\engines\\fake.exe");
        rt.ensure_stream("fen", &cfg, false);
        assert!(!rt.is_running());
    }

    #[test]
    fn ensure_stream_noop_when_path_empty() {
        let rt = EngineStreamRuntime::default();
        let cfg = sample_config("");
        rt.ensure_stream("fen", &cfg, true);
        assert!(!rt.is_running());
    }

    #[test]
    fn stop_stream_is_idempotent() {
        let rt = EngineStreamRuntime::default();
        rt.stop_stream();
        rt.stop_stream();
        assert!(!rt.is_running());
    }
}
