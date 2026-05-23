#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommand {
    New,
    Undo,
    Prev,
    Next,
    RedAi,
    BlackAi,
    Query,
    Rotate,
    Eval,
    CopyFen,
    PasteFen,
    Stop,
    Help,
    Exit,
    Quit,
}

impl SlashCommand {
    pub const ALL: [Self; 15] = [
        Self::New,
        Self::Undo,
        Self::Prev,
        Self::Next,
        Self::RedAi,
        Self::BlackAi,
        Self::Query,
        Self::Rotate,
        Self::Eval,
        Self::CopyFen,
        Self::PasteFen,
        Self::Stop,
        Self::Help,
        Self::Exit,
        Self::Quit,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::New => "/new",
            Self::Undo => "/undo",
            Self::Prev => "/prev",
            Self::Next => "/next",
            Self::RedAi => "/rai",
            Self::BlackAi => "/bai",
            Self::Query => "/query",
            Self::Rotate => "/rotate",
            Self::Eval => "/eval",
            Self::CopyFen => "/copyfen",
            Self::PasteFen => "/pastefen",
            Self::Stop => "/stop",
            Self::Help => "/help",
            Self::Exit => "/exit",
            Self::Quit => "/quit",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::New => "停止并开新棋",
            Self::Undo => "悔棋",
            Self::Prev => "上一步",
            Self::Next => "下一步",
            Self::RedAi => "切换红AI",
            Self::BlackAi => "切换黑AI",
            Self::Query => "切换查询模式",
            Self::Rotate => "旋转棋盘",
            Self::Eval => "切换实时评估",
            Self::CopyFen => "复制 FEN 到剪贴板",
            Self::PasteFen => "粘贴 FEN",
            Self::Stop => "停止模式、引擎流与自动走子",
            Self::Help => "操作说明",
            Self::Exit | Self::Quit => "退出软件",
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|command| command.name() == value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordinateMove {
    pub raw: String,
    pub from_file: u8,
    pub from_rank: u8,
    pub to_file: u8,
    pub to_rank: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Move(CoordinateMove),
    Slash(SlashCommand),
    /// `/pastefen` 后整段 FEN（含空格），单独解析避免与无参 slash 混淆。
    PasteFen(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandParseError {
    Empty,
    UnknownSlash(String),
    InvalidMove(String),
    InvalidPasteFen,
}

impl CommandParseError {
    pub fn message(&self) -> String {
        match self {
            Self::Empty => "输入为空。".to_string(),
            Self::UnknownSlash(command) => format!("未知命令：{command}"),
            Self::InvalidMove(value) => {
                format!("非法输入：{value}。普通输入必须满足 [a-i][0-9][a-i][0-9]。")
            }
            Self::InvalidPasteFen => "用法：/pastefen <FEN>（FEN 可含空格）。".to_string(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CommandService;

impl CommandService {
    pub fn parse(&self, input: &str) -> Result<ParsedCommand, CommandParseError> {
        let command = input.trim();
        if command.is_empty() {
            return Err(CommandParseError::Empty);
        }
        if command.starts_with('/') {
            return parse_slash_command(command);
        }
        parse_coordinate_move(&command.to_ascii_lowercase()).map(ParsedCommand::Move)
    }
}

/// 首个空白前为命令名（小写），之后为参数（FEN 原样保留大小写）。
fn parse_slash_command(input: &str) -> Result<ParsedCommand, CommandParseError> {
    let rest = input.trim_start_matches('/');
    let (name_part, args) = rest
        .split_once(char::is_whitespace)
        .map_or((rest, ""), |(n, a)| (n, a.trim()));
    let name = format!("/{}", name_part.to_ascii_lowercase());
    if name == SlashCommand::PasteFen.name() {
        if args.is_empty() {
            return Err(CommandParseError::InvalidPasteFen);
        }
        return Ok(ParsedCommand::PasteFen(args.to_string()));
    }
    SlashCommand::from_name(&name)
        .map(ParsedCommand::Slash)
        .ok_or(CommandParseError::UnknownSlash(input.trim().to_string()))
}

fn parse_coordinate_move(value: &str) -> Result<CoordinateMove, CommandParseError> {
    let bytes = value.as_bytes();
    if bytes.len() != 4
        || !matches!(bytes[0], b'a'..=b'i')
        || !bytes[1].is_ascii_digit()
        || !matches!(bytes[2], b'a'..=b'i')
        || !bytes[3].is_ascii_digit()
    {
        return Err(CommandParseError::InvalidMove(value.to_string()));
    }

    Ok(CoordinateMove {
        raw: value.to_string(),
        from_file: bytes[0] - b'a',
        from_rank: bytes[1] - b'0',
        to_file: bytes[2] - b'a',
        to_rank: bytes[3] - b'0',
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_coordinate_move_ok() {
        let parsed = CommandService
            .parse("h2e2")
            .expect("coordinate move should parse");
        match parsed {
            ParsedCommand::Move(mv) => {
                assert_eq!(mv.from_file, 7);
                assert_eq!(mv.from_rank, 2);
                assert_eq!(mv.to_file, 4);
                assert_eq!(mv.to_rank, 2);
            }
            ParsedCommand::Slash(_) | ParsedCommand::PasteFen(_) => panic!("expected move"),
        }
    }

    #[test]
    fn parse_slash_ok() {
        let parsed = CommandService.parse("/quit").expect("slash command");
        assert_eq!(parsed, ParsedCommand::Slash(SlashCommand::Quit));
    }

    #[test]
    fn parse_invalid_move_fails() {
        let err = CommandService.parse("a0j0").expect_err("invalid move");
        assert_eq!(err, CommandParseError::InvalidMove("a0j0".to_string()));
    }

    #[test]
    fn parse_pastefen_with_spaces() {
        let fen = "r3kabn1/4a2r1/1c2b4/pc1rp1c1p/2p1c4/6p2/p1r5p/8n/4k4/2balab2 w - - 0 1";
        let parsed = CommandService
            .parse(&format!("/pastefen {fen}"))
            .expect("pastefen");
        assert_eq!(parsed, ParsedCommand::PasteFen(fen.to_string()));
    }

    #[test]
    fn parse_pastefen_without_fen_fails() {
        let err = CommandService.parse("/pastefen").expect_err("need fen");
        assert_eq!(err, CommandParseError::InvalidPasteFen);
    }
}
