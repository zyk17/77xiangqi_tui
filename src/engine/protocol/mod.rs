//! 引擎文本协议：[`EngineProtocol`] 抽象 UCI/UCCI 握手差异；[`common`] 为共用 `info` 解析。

mod common;
pub mod ucci;
pub mod uci;

pub(crate) use common::parse_uci_style_info_tokens;
pub(crate) use common::{ParsedUciStyleInfo, candidate_from_parsed};

/// 与引擎子进程握手时选用的协议变体（`uci` / `ucci` 命令及就绪标记不同，后续 `setoption`/`position`/`go` 在本项目中按同一套文本发送）。
pub trait EngineProtocol {
    fn init_command(&self) -> &'static str;
    /// 引擎 stdout 中出现该子串即视为握手成功。
    fn handshake_done_token(&self) -> &'static str;
    /// 写入 `UciUcciEngine::last_protocol` 的标识（`uci` / `ucci`）。
    fn protocol_id(&self) -> &'static str;
}
