//! TUI 流式分析：后台 `go infinite`，主线程每帧读取 [`EngineAnalysisStore`]。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::engine::EngineConfig;
use crate::engine::analysis_store::EngineAnalysisStore;
use crate::engine::uci_ucci_engine::{EngineConfigureRequest, UciUcciEngine};
use crate::runtime_log;

fn lock_mutex<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 对齐 GUI `stop_stream_and_wait_on_sender` 的停流等待上限。
const STREAM_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

pub struct EngineStreamRuntime {
    engine: Arc<Mutex<UciUcciEngine>>,
    store: Arc<Mutex<EngineAnalysisStore>>,
    stop: Arc<AtomicBool>,
    join: Mutex<Option<JoinHandle<()>>>,
    autoplay_join: Mutex<Option<JoinHandle<crate::engine::EngineAnalyzeResult>>>,
    autoplay_cancel: Arc<AtomicBool>,
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
            autoplay_join: Mutex::new(None),
            autoplay_cancel: Arc::new(AtomicBool::new(false)),
            session_gen: Arc::new(AtomicU64::new(0)),
            active_fen: Mutex::new(String::new()),
        }
    }

    /// 后台 infinite 线程是否仍在运行（含停流等待中、尚未 join 的句柄）。
    fn infinite_thread_active(&self) -> bool {
        lock_mutex(&self.join)
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    }

    /// 无 infinite / AI 线程，但子进程仍存活时需释放（避免每帧重复 join+terminate）。
    pub fn needs_process_release(&self) -> bool {
        if self.infinite_thread_active() || self.is_autoplay_running() {
            return false;
        }
        self.engine
            .lock()
            .map(|eng| eng.has_child_process())
            .unwrap_or(false)
    }

    /// 无消费者时终止子进程（对齐 GUI `prepare_for_next_engine_command`：先 join 再 terminate）。
    pub fn release_engine_process(&self) {
        if !self.needs_process_release() {
            return;
        }
        self.stop_infinite_stream_blocking();
        self.stop_autoplay_blocking();
        if let Ok(eng) = self.engine.lock() {
            eng.terminate_locked();
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
        self.infinite_thread_active()
    }

    pub fn active_fen(&self) -> String {
        lock_mutex(&self.active_fen).clone()
    }

    /// 启动或重启流式分析（局面或配置变化时调用）。
    pub fn ensure_stream(&self, fen: &str, cfg: &EngineConfig, want_stream: bool) {
        if !want_stream || cfg.path.trim().is_empty() {
            if self.infinite_thread_active() {
                self.stop_infinite_stream();
            }
            return;
        }
        let fen = fen.trim().to_string();
        if self.infinite_thread_active() {
            if self.active_fen() == fen {
                return;
            }
            self.stop_infinite_stream_blocking();
        }
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

    /// 仅停 `go infinite`；**不**等待/取消 AI `go`（对齐 GUI `stop_infinite_analysis_stream`）。
    pub fn stop_infinite_stream(&self) {
        self.stop_infinite_stream_inner(STREAM_JOIN_TIMEOUT);
    }

    fn stop_infinite_stream_blocking(&self) {
        self.stop_infinite_stream_inner(STREAM_JOIN_TIMEOUT + Duration::from_secs(3));
    }

    fn stop_infinite_stream_inner(&self, join_timeout: Duration) {
        self.stop.store(true, Ordering::SeqCst);
        self.session_gen.fetch_add(1, Ordering::SeqCst);
        let deadline = Instant::now() + join_timeout;
        loop {
            let mut slot = lock_mutex(&self.join);
            let Some(handle) = slot.take() else {
                break;
            };
            if handle.is_finished() {
                drop(slot);
                let _ = handle.join();
                continue;
            }
            if Instant::now() >= deadline {
                *slot = Some(handle);
                runtime_log::warn(
                    "[engine_stream] infinite join timed out; stream stays inactive until thread exits",
                );
                break;
            }
            drop(slot);
            thread::sleep(Duration::from_millis(10));
        }
        self.stop.store(false, Ordering::SeqCst);
        *lock_mutex(&self.active_fen) = String::new();
    }

    /// 停 infinite + 等待 AI 一次性 `go` 结束（新局、`/stop`、退出）。
    pub fn stop_all(&self) {
        self.stop_infinite_stream_blocking();
        self.stop_autoplay_blocking();
    }

    pub fn is_autoplay_running(&self) -> bool {
        let guard = lock_mutex(&self.autoplay_join);
        guard.as_ref().is_some_and(|handle| !handle.is_finished())
    }

    pub fn spawn_autoplay_once(&self, fen: &str, cfg: &EngineConfig) {
        self.stop_infinite_stream();
        self.stop_autoplay();
        self.autoplay_cancel.store(false, Ordering::SeqCst);
        let fen = fen.trim().to_string();
        {
            let mut guard = lock_mutex(&self.store);
            guard.reset_for_stream(&fen);
        }
        self.configure_engine(cfg);
        let eng = self.engine.clone();
        let store = self.store.clone();
        let cfg = cfg.clone();
        let cancel = self.autoplay_cancel.clone();
        let handle = thread::spawn(move || {
            if let Ok(mut engine) = eng.lock() {
                let (depth, movetime_ms, search_nodes) = cfg.analyze_go_args();
                engine.analyze_autoplay_once_with_cancel(
                    fen.as_str(),
                    depth,
                    movetime_ms,
                    search_nodes,
                    Some(&store),
                    Some(&cancel),
                )
            } else {
                crate::engine::EngineAnalyzeResult::default()
            }
        });
        *lock_mutex(&self.autoplay_join) = Some(handle);
    }

    /// 若后台 `go` 已结束则取走结果；思考中返回 `None`。
    pub fn poll_autoplay_done(&self) -> Option<crate::engine::EngineAnalyzeResult> {
        let mut slot = lock_mutex(&self.autoplay_join);
        let handle = slot.as_ref()?;
        if !handle.is_finished() {
            return None;
        }
        let handle = slot.take()?;
        handle.join().ok()
    }

    pub fn stop_autoplay(&self) {
        self.autoplay_cancel.store(true, Ordering::SeqCst);
        let handle = lock_mutex(&self.autoplay_join).take();
        if let Some(handle) = handle {
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                thread::spawn(move || {
                    let _ = handle.join();
                });
            }
        }
        self.autoplay_cancel.store(false, Ordering::SeqCst);
    }

    /// 退出应用时同步等待 AI `go` 结束。
    pub fn stop_autoplay_blocking(&self) {
        self.autoplay_cancel.store(true, Ordering::SeqCst);
        if let Some(handle) = lock_mutex(&self.autoplay_join).take() {
            let _ = handle.join();
        }
        self.autoplay_cancel.store(false, Ordering::SeqCst);
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
            ..EngineConfig::default()
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
    fn stop_infinite_stream_is_idempotent() {
        let rt = EngineStreamRuntime::default();
        rt.stop_infinite_stream();
        rt.stop_infinite_stream();
        assert!(!rt.is_running());
    }
}
