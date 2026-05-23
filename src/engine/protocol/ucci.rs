//! **UCCI** 协议：初始化命令与握手完成标记。

use super::EngineProtocol;

#[derive(Debug, Clone, Copy, Default)]
pub struct Ucci;

impl EngineProtocol for Ucci {
    fn init_command(&self) -> &'static str {
        "ucci"
    }

    fn handshake_done_token(&self) -> &'static str {
        "ucciok"
    }

    fn protocol_id(&self) -> &'static str {
        "ucci"
    }
}
