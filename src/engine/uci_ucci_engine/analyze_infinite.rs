use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::analyze_infinite_line::{
    apply_infinite_stdout_line, patch_store_from_state, InfiniteLineOutcome,
};
use super::engine_core::UciUcciEngine;
use super::info_state::EngineInfoState;
use super::types::EngineStdoutPoll;
use super::ui_helpers::stub_result;
use crate::engine::analysis_store::EngineAnalysisStore;
use crate::runtime_log;

pub(crate) const INFINITE_STDOUT_POLL_MS: u64 = 50;

fn lock_store(store: &Arc<Mutex<EngineAnalysisStore>>) -> std::sync::MutexGuard<'_, EngineAnalysisStore> {
    store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn session_live(live_session: &Arc<AtomicU64>, session_id: u64) -> bool {
    live_session.load(Ordering::SeqCst) == session_id
}

impl UciUcciEngine {
    pub(crate) fn run_infinite_analysis(
        &mut self,
        fen: &str,
        store: &Arc<Mutex<EngineAnalysisStore>>,
        stop: &Arc<AtomicBool>,
        live_session: &Arc<AtomicU64>,
        session_id: u64,
        multi_pv: i32,
    ) {
        let mpv = multi_pv.clamp(1, 5);
        if self.engine_path.is_none() {
            let mut guard = lock_store(store);
            guard.fen = fen.trim().to_string();
            guard.result = stub_result();
            return;
        }
        if self.rt.lock().map(|g| g.is_none()).unwrap_or(true) {
            self.start();
        }
        if self.rt.lock().map(|g| g.is_none()).unwrap_or(true) {
            let mut guard = lock_store(store);
            guard.fen = fen.trim().to_string();
            guard.result = stub_result();
            return;
        }
        if !session_live(live_session, session_id) {
            return;
        }
        {
            let mut guard = lock_store(store);
            guard.reset_for_stream(fen);
        }
        let _ = self.send_cmd("stop");
        if let Err(e) = self.send_cmd(&format!("setoption name MultiPV value {mpv}")) {
            runtime_log::warn(format!(
                "[engine_infinite] send_err stage=set_multipv err={e}"
            ));
            self.terminate_locked();
            return;
        }
        if !fen.trim().is_empty() {
            if let Err(e) = self.send_cmd(&format!("position fen {}", fen.trim())) {
                runtime_log::warn(format!(
                    "[engine_infinite] send_err stage=position_fen err={e}"
                ));
                self.terminate_locked();
                return;
            }
        } else if let Err(e) = self.send_cmd("position startpos") {
            runtime_log::warn(format!(
                "[engine_infinite] send_err stage=position_startpos err={e}"
            ));
            self.terminate_locked();
            return;
        }
        self.clear_queue();
        if let Err(e) = self.send_cmd("go infinite") {
            runtime_log::warn(format!(
                "[engine_infinite] send_err stage=go_infinite err={e}"
            ));
            self.terminate_locked();
            return;
        }
        let mut st = EngineInfoState::new();
        let mut got_best = false;
        let mut stop_sent = false;
        let mut stop_at = Instant::now();
        while !got_best {
            if !session_live(live_session, session_id) {
                break;
            }
            if stop.load(Ordering::SeqCst) {
                if !stop_sent {
                    let _ = self.send_cmd("stop");
                    stop_sent = true;
                    stop_at = Instant::now();
                }
                if stop_sent && stop_at.elapsed() > Duration::from_secs(3) {
                    break;
                }
            }
            match self.poll_line(Duration::from_millis(INFINITE_STDOUT_POLL_MS)) {
                EngineStdoutPoll::Disconnected { .. } => break,
                EngineStdoutPoll::Tick => {}
                EngineStdoutPoll::Line(line) => {
                    if !session_live(live_session, session_id) {
                        break;
                    }
                    match apply_infinite_stdout_line(&line, fen, &mut st) {
                        InfiniteLineOutcome::Continue => {
                            let mut guard = lock_store(store);
                            patch_store_from_state(&mut guard, fen, &st);
                        }
                        InfiniteLineOutcome::GotBestmove => {
                            let mut guard = lock_store(store);
                            patch_store_from_state(&mut guard, fen, &st);
                            guard.patch_best_move(st.best_move.clone());
                            got_best = true;
                        }
                    }
                }
            }
        }
        if !got_best && !stop.load(Ordering::SeqCst) {
            runtime_log::warn("[engine_infinite] bestmove_not_observed_before_exit");
        }
        if session_live(live_session, session_id)
            && !st.best_move.is_empty()
            && st.best_move != "stub_move"
        {
            lock_store(store).patch_best_move(st.best_move);
        }
    }
}
