//! 开局库后台查询：单 worker 串行执行，新请求覆盖旧请求，不堆积 join 辅助线程。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::book::{BookConfig, BookResponse, query_opening_book};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookQueryKind {
    /// D 区展示 / 阻挡引擎流
    Display,
    /// AI 自动走子前先查库
    Autoplay,
}

struct BookJob {
    generation: u64,
    fen: String,
    kind: BookQueryKind,
    cfg: BookConfig,
}

struct CompletedBook {
    generation: u64,
    fen: String,
    kind: BookQueryKind,
    response: BookResponse,
}

fn book_worker(
    rx: Receiver<BookJob>,
    result: Arc<Mutex<Option<CompletedBook>>>,
    generation: Arc<AtomicU64>,
) {
    while let Ok(mut job) = rx.recv() {
        while let Ok(next) = rx.try_recv() {
            job = next;
        }
        let response = query_opening_book(&job.fen, None, &job.cfg, false);
        if job.generation == generation.load(Ordering::SeqCst) {
            *lock(&result) = Some(CompletedBook {
                generation: job.generation,
                fen: job.fen,
                kind: job.kind,
                response,
            });
        }
    }
}

pub struct BookQueryRuntime {
    tx: Sender<BookJob>,
    /// 与 worker 共享；`spawn` / `cancel` 与 worker 完成回调必须读写同一实例。
    generation: Arc<AtomicU64>,
    result: Arc<Mutex<Option<CompletedBook>>>,
    /// 已派发、尚未写入 `result` 的任务（用于 `pending_fen` / `is_busy`）。
    inflight: Mutex<Option<(u64, String, BookQueryKind)>>,
}

impl Default for BookQueryRuntime {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        let result = Arc::new(Mutex::new(None));
        let generation = Arc::new(AtomicU64::new(0));
        let gen_worker = generation.clone();
        let result_worker = result.clone();
        thread::spawn(move || book_worker(rx, result_worker, gen_worker));
        Self {
            tx,
            generation,
            result,
            inflight: Mutex::new(None),
        }
    }
}

impl std::fmt::Debug for BookQueryRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BookQueryRuntime")
            .field("busy", &self.is_busy())
            .finish()
    }
}

impl BookQueryRuntime {
    fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn result_ready_for(&self, job_gen: u64) -> bool {
        lock(&self.result)
            .as_ref()
            .is_some_and(|r| r.generation >= job_gen)
    }

    pub fn is_busy(&self) -> bool {
        let want = self.current_generation();
        if want == 0 {
            return false;
        }
        if self.result_ready_for(want) {
            return false;
        }
        lock(&self.inflight).is_some()
    }

    pub fn pending_fen(&self) -> Option<String> {
        let guard = lock(&self.inflight);
        guard.as_ref().map(|(_, fen, _)| fen.clone())
    }

    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        *lock(&self.result) = None;
        *lock(&self.inflight) = None;
    }

    /// 若尚无同 FEN 在途查询，则交给后台 worker。
    pub fn spawn_if_needed(&self, fen: &str, cfg: &BookConfig, kind: BookQueryKind) {
        let fen = fen.trim().to_string();
        if fen.is_empty() {
            return;
        }
        {
            let inflight = lock(&self.inflight);
            if let Some((job_gen, pending_fen, pending_kind)) = inflight.as_ref()
                && pending_fen == &fen && *pending_kind == kind && !self.result_ready_for(*job_gen) {
                    return;
                }
        }
        let job_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *lock(&self.result) = None;
        *lock(&self.inflight) = Some((job_gen, fen.clone(), kind));
        let _ = self.tx.send(BookJob {
            generation: job_gen,
            fen,
            kind,
            cfg: cfg.clone(),
        });
    }

    /// 查询完成后取走结果；进行中返回 `None`。
    pub fn poll(&self) -> Option<(String, BookQueryKind, BookResponse)> {
        let completed = lock(&self.result).take()?;
        let want = self.current_generation();
        if completed.generation < want {
            return None;
        }
        *lock(&self.inflight) = None;
        Some((completed.fen, completed.kind, completed.response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_sees_same_generation_as_runtime() {
        let rt = BookQueryRuntime::default();
        rt.generation.fetch_add(1, Ordering::SeqCst);
        assert_eq!(rt.current_generation(), 1);
        rt.generation.fetch_add(1, Ordering::SeqCst);
        assert_eq!(rt.current_generation(), 2);
    }
}
