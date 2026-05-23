//! TUI 版引擎层：只保留当前项目需要的最小主干。
//! - `protocol`: UCI/UCCI 文本协议与 `info` 解析
//! - `uci_ucci_engine`: 子进程、握手、一次分析、无限分析
//!
//! 明确移除：
//! - 智能时间 / 校准
//! - 强制变招
//! - Tauri / JSON 数据桥接

pub mod analysis_types;
pub mod protocol;
pub mod pv_ui;
pub mod uci_ucci_engine;

pub use analysis_types::{EngineAnalyzeResult, EngineInfoCandidate};

pub use uci_ucci_engine::UciUcciEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineProtocol {
    Uci,
    Ucci,
}

impl EngineProtocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::Uci => "UCI",
            Self::Ucci => "UCCI",
        }
    }

    pub fn preference(self) -> &'static str {
        match self {
            Self::Uci => "uci_only",
            Self::Ucci => "ucci_only",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub path: String,
    pub protocol: EngineProtocol,
    pub threads: u8,
    pub hash_mb: u32,
    pub skill_level: u8,
    pub multi_pv: u8,
    pub variant: String,
    pub rule: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            protocol: EngineProtocol::Uci,
            threads: 4,
            hash_mb: 512,
            skill_level: 20,
            multi_pv: 1,
            variant: "AsianRule".to_string(),
            rule: "None".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisSnapshot {
    pub time_text: String,
    pub depth: u16,
    pub nps: u64,
    pub nodes: u64,
    pub score_text: String,
    pub best_move: String,
    pub win_rate_text: String,
    pub pv: Vec<String>,
    pub source: String,
}

impl AnalysisSnapshot {
    pub fn idle() -> Self {
        Self {
            time_text: "--".to_string(),
            depth: 0,
            nps: 0,
            nodes: 0,
            score_text: "--".to_string(),
            best_move: "--".to_string(),
            win_rate_text: "--".to_string(),
            pv: Vec::new(),
            source: "idle".to_string(),
        }
    }
}
