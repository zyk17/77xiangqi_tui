#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Red,
    Black,
}

impl Side {
    pub fn from_fen_turn_field(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "b" => Side::Black,
            _ => Side::Red,
        }
    }

    pub fn fen_turn_char(self) -> char {
        match self {
            Side::Red => 'w',
            Side::Black => 'b',
        }
    }

    pub fn is_red(self) -> bool {
        matches!(self, Side::Red)
    }

    pub fn other(self) -> Self {
        match self {
            Side::Red => Side::Black,
            Side::Black => Side::Red,
        }
    }
}
