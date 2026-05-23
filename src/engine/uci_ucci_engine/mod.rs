//! 子进程象棋引擎：保留 TUI 当前需要的最小主干。

mod analyze_infinite;
mod analyze_infinite_line;
mod analyze_once;
mod engine_core;
mod engine_path;
mod handshake;
mod handshake_plan;
pub(crate) mod info_state;
mod process;
mod types;
pub mod ui_helpers;

#[cfg(test)]
mod test_hook;

pub use engine_core::UciUcciEngine;
pub use types::EngineConfigureRequest;
