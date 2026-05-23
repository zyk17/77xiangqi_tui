//! 握手协议尝试顺序（纯逻辑，与 [`super::handshake`] 子进程实现分离）。

/// 按 `engine_protocol_preference` 与路径提示返回依次尝试的协议 id（`uci` / `ucci`）。
pub(crate) fn handshake_protocol_sequence(pref: &str, hint: Option<&str>) -> Vec<&'static str> {
    match pref {
        "uci_only" => vec!["uci"],
        "ucci_only" => vec!["ucci"],
        "auto" | "prefer_ucci" => match hint {
            Some("uci") => vec!["uci", "ucci"],
            Some("ucci") => vec!["ucci", "uci"],
            _ => vec!["ucci", "uci"],
        },
        "prefer_uci" => match hint {
            Some("ucci") => vec!["ucci", "uci"],
            _ => vec!["uci", "ucci"],
        },
        _ => match hint {
            Some("uci") => vec!["uci", "ucci"],
            Some("ucci") => vec!["ucci", "uci"],
            _ => vec!["ucci", "uci"],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_defaults_ucci_first() {
        assert_eq!(
            handshake_protocol_sequence("auto", None),
            vec!["ucci", "uci"]
        );
    }

    #[test]
    fn prefer_uci_with_hint_ucci_tries_ucci_first() {
        assert_eq!(
            handshake_protocol_sequence("prefer_uci", Some("ucci")),
            vec!["ucci", "uci"]
        );
    }

    #[test]
    fn uci_only_single_attempt() {
        assert_eq!(handshake_protocol_sequence("uci_only", None), vec!["uci"]);
    }
}
