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
    Exit,
    Quit,
}

impl SlashCommand {
    pub const ALL: [Self; 13] = [
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
            Self::Exit => "/exit",
            Self::Quit => "/quit",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::New => "新游戏",
            Self::Undo => "悔棋",
            Self::Prev => "上一步",
            Self::Next => "下一步",
            Self::RedAi => "切换红AI",
            Self::BlackAi => "切换黑AI",
            Self::Query => "切换查询模式",
            Self::Rotate => "旋转棋盘",
            Self::Eval => "切换实时评估",
            Self::CopyFen => "复制FEN",
            Self::PasteFen => "粘贴FEN",
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandParseError {
    Empty,
    UnknownSlash(String),
    InvalidMove(String),
}

impl CommandParseError {
    pub fn message(&self) -> String {
        match self {
            Self::Empty => "输入为空。".to_string(),
            Self::UnknownSlash(command) => format!("未知命令：{command}"),
            Self::InvalidMove(value) => {
                format!("非法输入：{value}。普通输入必须满足 [a-i][0-9][a-i][0-9]。")
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CommandService;

impl CommandService {
    pub fn parse(&self, input: &str) -> Result<ParsedCommand, CommandParseError> {
        let command = input.trim().to_ascii_lowercase();
        if command.is_empty() {
            return Err(CommandParseError::Empty);
        }
        if command.starts_with('/') {
            return SlashCommand::from_name(&command)
                .map(ParsedCommand::Slash)
                .ok_or(CommandParseError::UnknownSlash(command));
        }
        parse_coordinate_move(&command).map(ParsedCommand::Move)
    }
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
            ParsedCommand::Slash(_) => panic!("expected move"),
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
}
