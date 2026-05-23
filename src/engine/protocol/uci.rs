//! **UCI** 协议：初始化命令与握手完成标记。

use super::EngineProtocol;

#[derive(Debug, Clone, Copy, Default)]
pub struct Uci;

impl EngineProtocol for Uci {
    fn init_command(&self) -> &'static str {
        "uci"
    }

    fn handshake_done_token(&self) -> &'static str {
        "uciok"
    }

    fn protocol_id(&self) -> &'static str {
        "uci"
    }
}
