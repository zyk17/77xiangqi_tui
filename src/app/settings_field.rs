use crate::engine::EngineProtocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsFieldKind {
    Text,
    Bool,
    Cycle,
    Number,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    EnginePath,
    EngineProtocol,
    EngineThreads,
    EngineHashMb,
    EngineSkill,
    EngineMultiPv,
    BookLocalPath,
    BookLocalEnabled,
    BookCloudEnabled,
    BookPickMode,
    BookMaxHalfmoves,
}

impl SettingsField {
    pub const ALL: [SettingsField; 11] = [
        Self::EnginePath,
        Self::EngineProtocol,
        Self::EngineThreads,
        Self::EngineHashMb,
        Self::EngineSkill,
        Self::EngineMultiPv,
        Self::BookLocalPath,
        Self::BookLocalEnabled,
        Self::BookCloudEnabled,
        Self::BookPickMode,
        Self::BookMaxHalfmoves,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::EnginePath => "引擎路径",
            Self::EngineProtocol => "协议",
            Self::EngineThreads => "线程数",
            Self::EngineHashMb => "Hash(MB)",
            Self::EngineSkill => "棋力 Skill",
            Self::EngineMultiPv => "MultiPV",
            Self::BookLocalPath => "本地库路径",
            Self::BookLocalEnabled => "启用本地库",
            Self::BookCloudEnabled => "启用云库",
            Self::BookPickMode => "库招选取",
            Self::BookMaxHalfmoves => "开局库最多步",
        }
    }

    pub fn kind(self) -> SettingsFieldKind {
        match self {
            Self::EnginePath | Self::BookLocalPath => SettingsFieldKind::Text,
            Self::BookLocalEnabled | Self::BookCloudEnabled => SettingsFieldKind::Bool,
            Self::EngineProtocol | Self::BookPickMode => SettingsFieldKind::Cycle,
            Self::EngineThreads
            | Self::EngineHashMb
            | Self::EngineSkill
            | Self::EngineMultiPv
            | Self::BookMaxHalfmoves => SettingsFieldKind::Number,
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::EnginePath => "Enter 在 C 区输入路径，Esc 返回。",
            Self::BookLocalPath => "Enter 在 C 区输入库路径，Esc 返回。",
            Self::EngineProtocol => "←/→ 或 Enter 在 C 区输入 uci/ucci。",
            Self::BookPickMode => "←/→ 切换；Enter 输入 optimal/positive_random。",
            Self::BookLocalEnabled | Self::BookCloudEnabled => "空格切换；Enter 输入 0/1。",
            Self::EngineThreads => "←/→ 微调；Enter 在 C 区输入数字（1～64）。",
            Self::EngineHashMb => "←/→ 微调；Enter 在 C 区输入 MB（64～8192）。",
            Self::EngineSkill => "←/→ 微调；Enter 在 C 区输入（0～20）。",
            Self::EngineMultiPv => "←/→ 微调；Enter 在 C 区输入（1～5）。",
            Self::BookMaxHalfmoves => "←/→ 微调；Enter 在 C 区输入步数（0=不用库）。",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let index = Self::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

pub fn pick_mode_label(mode: &str) -> &'static str {
    if mode == "positive_random" {
        "正向随机"
    } else {
        "最优"
    }
}

pub fn cycle_pick_mode(mode: &str, delta: isize) -> String {
    let modes = ["optimal", "positive_random"];
    let index = modes.iter().position(|m| *m == mode).unwrap_or(0) as isize;
    let next = (index + delta).rem_euclid(modes.len() as isize) as usize;
    modes[next].to_string()
}

pub fn cycle_protocol(protocol: EngineProtocol, delta: isize) -> EngineProtocol {
    if delta.rem_euclid(2) == 0 {
        return protocol;
    }
    match protocol {
        EngineProtocol::Uci => EngineProtocol::Ucci,
        EngineProtocol::Ucci => EngineProtocol::Uci,
    }
}

pub fn clamp_threads(v: i32) -> u8 {
    v.clamp(1, 64) as u8
}

pub fn bump_hash_mb(current: u32, delta: isize) -> u32 {
    let steps = [64_u32, 128, 256, 512, 1024, 2048, 4096, 8192];
    let index = steps.iter().position(|s| *s == current).unwrap_or(3);
    let next = (index as isize + delta).clamp(0, steps.len() as isize - 1) as usize;
    steps[next]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_pick_mode_roundtrip() {
        assert_eq!(cycle_pick_mode("optimal", 1), "positive_random");
        assert_eq!(cycle_pick_mode("positive_random", 1), "optimal");
    }
}
