//! 子进程象棋引擎：保留 TUI 当前需要的最小主干。

mod analyze_once;
mod engine_core;
mod engine_path;
mod handshake;
mod handshake_plan;
mod info_state;
mod process;
mod types;
mod ui_helpers;

#[cfg(test)]
mod test_hook;

pub use engine_core::UciUcciEngine;
pub(crate) use engine_path::same_engine_path;
pub use types::{EngineAnalyzeRequest, EngineConfigureRequest};
pub use ui_helpers::{move_human_from_fen, red_black_winrate_pct_from_wdl};
