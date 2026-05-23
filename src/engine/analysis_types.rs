//! 引擎分析结果：TUI 内存态，不用 JSON。

/// 单条 MultiPV / `info` 候选。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EngineInfoCandidate {
    pub rank: i32,
    pub best_move: String,
    pub score: f64,
    pub score_cp: Option<i32>,
    pub mate: Option<i32>,
    pub pv: Vec<String>,
    pub depth: Option<i32>,
    pub nodes: Option<u64>,
    pub wdl: Option<[u64; 3]>,
}

/// 一次 `go depth/movetime/nodes` 分析的最终结果。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EngineAnalyzeResult {
    pub best_move: String,
    pub score: f64,
    pub score_cp: Option<i64>,
    pub pv: Vec<String>,
    pub depth: Option<i32>,
    pub candidates: Vec<EngineInfoCandidate>,
    pub search_time_ms: Option<u64>,
    pub nps: Option<u64>,
    pub nodes: Option<u64>,
    pub wdl: Option<[u64; 3]>,
    pub mate: Option<i32>,
}

impl EngineAnalyzeResult {
    pub fn stub() -> Self {
        Self {
            best_move: String::new(),
            score: 0.0,
            score_cp: None,
            pv: Vec::new(),
            depth: Some(0),
            candidates: Vec::new(),
            search_time_ms: Some(0),
            nps: Some(0),
            nodes: Some(0),
            wdl: None,
            mate: None,
        }
    }
}
