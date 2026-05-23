//! TUI 版引擎层：只保留当前项目需要的最小主干。
//! - `protocol`: UCI/UCCI 文本协议与 `info` 解析
//! - `uci_ucci_engine`: 子进程、握手、一次分析、无限分析
//!
//! 明确移除：
//! - 智能时间 / 校准
//! - 强制变招
//! - Tauri / JSON 数据桥接

pub mod analysis_store;
pub mod analysis_types;
pub mod protocol;
pub mod pv_ui;
pub mod stream;
pub mod uci_ucci_engine;

pub use analysis_store::EngineAnalysisStore;
pub use analysis_types::EngineAnalyzeResult;
pub use stream::EngineStreamRuntime;

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

/// 单次 `go` 限制方式（对齐 GUI `engine_autoplay_limit_mode` 的 movetime/depth/nodes）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineSearchLimit {
    #[default]
    Movetime,
    Depth,
    Nodes,
}

impl EngineSearchLimit {
    pub const ALL: [Self; 3] = [Self::Movetime, Self::Depth, Self::Nodes];

    pub fn label(self) -> &'static str {
        match self {
            Self::Movetime => "固定时间",
            Self::Depth => "固定深度",
            Self::Nodes => "固定节点",
        }
    }

    pub fn config_key(self) -> &'static str {
        match self {
            Self::Movetime => "movetime",
            Self::Depth => "depth",
            Self::Nodes => "nodes",
        }
    }

    pub fn from_config_key(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "depth" => Self::Depth,
            "nodes" => Self::Nodes,
            _ => Self::Movetime,
        }
    }

    pub fn cycle(self, delta: isize) -> Self {
        let modes = Self::ALL;
        let index = modes.iter().position(|m| *m == self).unwrap_or(0) as isize;
        let next = (index + delta).rem_euclid(modes.len() as isize) as usize;
        modes[next]
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
    pub search_limit: EngineSearchLimit,
    /// `go movetime`（毫秒）。
    pub movetime_ms: u32,
    /// `go depth`。
    pub search_depth: u8,
    /// `go nodes`。
    pub search_nodes: u32,
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
            search_limit: EngineSearchLimit::Movetime,
            movetime_ms: 3000,
            search_depth: 12,
            search_nodes: 500_000,
            variant: "AsianRule".to_string(),
            rule: "None".to_string(),
        }
    }
}

impl EngineConfig {
    /// 供 `analyze_once` 使用的 `go` 参数（互斥：movetime > nodes > depth）。
    pub fn analyze_go_args(&self) -> (Option<i32>, Option<i32>, Option<i32>) {
        match self.search_limit {
            EngineSearchLimit::Movetime => {
                let mt = i32::try_from(self.movetime_ms.max(1)).unwrap_or(3000);
                (None, Some(mt), None)
            }
            EngineSearchLimit::Depth => {
                let d = i32::from(self.search_depth.max(1));
                (Some(d), None, None)
            }
            EngineSearchLimit::Nodes => {
                let n = i32::try_from(self.search_nodes.max(1_000)).unwrap_or(500_000);
                (None, None, Some(n))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_go_args_follow_search_limit() {
        let movetime = EngineConfig {
            search_limit: EngineSearchLimit::Movetime,
            movetime_ms: 1500,
            ..EngineConfig::default()
        };
        assert_eq!(movetime.analyze_go_args(), (None, Some(1500), None));

        let depth = EngineConfig {
            search_limit: EngineSearchLimit::Depth,
            search_depth: 18,
            ..EngineConfig::default()
        };
        assert_eq!(depth.analyze_go_args(), (Some(18), None, None));

        let nodes = EngineConfig {
            search_limit: EngineSearchLimit::Nodes,
            search_nodes: 2_000_000,
            ..EngineConfig::default()
        };
        assert_eq!(nodes.analyze_go_args(), (None, None, Some(2_000_000)));
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
