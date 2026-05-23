//! 开局库后台查询（对齐 GUI `query_opening_book` 异步，避免主线程卡 UI）。

use std::sync::Mutex;
use std::thread::{self, JoinHandle};

use crate::book::{BookConfig, BookResponse, query_opening_book};

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookQueryKind {
    /// D 区展示 / 阻挡引擎流
    Display,
    /// AI 自动走子前先查库
    Autoplay,
}

struct PendingBookQuery {
    fen: String,
    kind: BookQueryKind,
    handle: JoinHandle<BookResponse>,
}

#[derive(Default)]
pub struct BookQueryRuntime {
    pending: Mutex<Option<PendingBookQuery>>,
}

impl std::fmt::Debug for BookQueryRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BookQueryRuntime")
            .field("busy", &self.is_busy())
            .finish()
    }
}

impl BookQueryRuntime {
    pub fn is_busy(&self) -> bool {
        let guard = lock(&self.pending);
        guard.as_ref().is_some_and(|p| !p.handle.is_finished())
    }

    pub fn pending_fen(&self) -> Option<String> {
        let guard = lock(&self.pending);
        guard.as_ref().map(|p| p.fen.clone())
    }

    pub fn cancel(&self) {
        let pending = lock(&self.pending).take();
        if let Some(p) = pending {
            if !p.handle.is_finished() {
                thread::spawn(move || {
                    let _ = p.handle.join();
                });
            }
        }
    }

    /// 若尚无同 FEN 在途查询，则后台 `query_opening_book`。
    pub fn spawn_if_needed(&self, fen: &str, cfg: &BookConfig, kind: BookQueryKind) {
        let fen = fen.trim().to_string();
        if fen.is_empty() {
            return;
        }
        let mut slot = lock(&self.pending);
        if let Some(p) = slot.as_ref() {
            if p.fen == fen && p.kind == kind && !p.handle.is_finished() {
                return;
            }
        }
        if let Some(p) = slot.take() {
            if !p.handle.is_finished() {
                thread::spawn(move || {
                    let _ = p.handle.join();
                });
            }
        }
        let cfg = cfg.clone();
        let fen_for_thread = fen.clone();
        let handle = thread::spawn(move || query_opening_book(&fen_for_thread, None, &cfg, false));
        *slot = Some(PendingBookQuery { fen, kind, handle });
    }

    /// 查询完成后取走结果；进行中返回 `None`。
    pub fn poll(&self) -> Option<(String, BookQueryKind, BookResponse)> {
        let mut slot = lock(&self.pending);
        let pending = slot.as_ref()?;
        if !pending.handle.is_finished() {
            return None;
        }
        let pending = slot.take()?;
        let response = pending.handle.join().ok()?;
        Some((pending.fen, pending.kind, response))
    }
}
