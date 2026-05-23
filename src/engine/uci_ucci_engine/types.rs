use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct EngineConfigureRequest {
    pub engine_path: Option<String>,
    pub threads: Option<i32>,
    pub hash_mb: Option<i32>,
    pub repetition_rule: Option<String>,
    pub draw_rule: Option<String>,
    pub skill_level: Option<i32>,
    pub engine_protocol_preference: Option<String>,
    pub engine_config_path: Option<PathBuf>,
    pub protocol_detected_for_path: Option<String>,
    pub protocol_detected: Option<String>,
}

pub struct EngineAnalyzeRequest<'a> {
    pub fen: &'a str,
    pub depth: Option<i32>,
    pub movetime_ms: Option<i32>,
    pub search_moves: Option<&'a [String]>,
    pub search_nodes: Option<i32>,
    pub multipv_override: Option<i32>,
    pub cancel: Option<Arc<AtomicBool>>,
}

/// [`super::process::UciUcciEngine::poll_line`]：单行、空闲 tick，或 I/O 结束。
pub(crate) enum EngineStdoutPoll {
    Line(String),
    Tick,
    Disconnected { child_status: String },
}
