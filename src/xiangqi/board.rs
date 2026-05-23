pub const STARTPOS_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR r";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board90 {
    pub cells: [u8; 90],
}

impl Default for Board90 {
    fn default() -> Self {
        Self { cells: [b'.'; 90] }
    }
}

impl Board90 {
    #[inline]
    pub fn startpos() -> Self {
        Self::from_fen(STARTPOS_FEN).unwrap_or_default()
    }

    #[inline]
    pub fn index(file: u8, rank: u8) -> usize {
        (rank as usize * 9) + file as usize
    }

    #[inline]
    pub fn get(&self, file: u8, rank: u8) -> u8 {
        self.cells[Self::index(file, rank)]
    }

    #[inline]
    pub fn set(&mut self, file: u8, rank: u8, piece: u8) {
        self.cells[Self::index(file, rank)] = piece;
    }

    pub fn display_at(&self, file: u8, rank: u8) -> char {
        self.get(file, rank) as char
    }

    pub fn to_fen(&self) -> String {
        let mut rows = Vec::with_capacity(10);
        for rank in 0..10_u8 {
            let mut row = String::new();
            let mut empty = 0_u8;
            for file in 0..9_u8 {
                let piece = self.get(file, rank);
                if piece == b'.' {
                    empty += 1;
                    continue;
                }
                if empty != 0 {
                    row.push(char::from(b'0' + empty));
                    empty = 0;
                }
                row.push(piece as char);
            }
            if empty != 0 {
                row.push(char::from(b'0' + empty));
            }
            rows.push(row);
        }
        format!("{} r", rows.join("/"))
    }

    pub fn from_fen(fen: &str) -> Option<Self> {
        let board_part = fen.split_whitespace().next()?;
        let mut board = Self::default();
        let mut rank = 0_u8;
        let mut file = 0_u8;

        for ch in board_part.chars() {
            match ch {
                '/' => {
                    if file != 9 {
                        return None;
                    }
                    rank += 1;
                    file = 0;
                }
                '1'..='9' => {
                    let count = ch.to_digit(10)? as u8;
                    for _ in 0..count {
                        if file >= 9 || rank >= 10 {
                            return None;
                        }
                        board.set(file, rank, b'.');
                        file += 1;
                    }
                }
                piece if piece.is_ascii_alphabetic() => {
                    if file >= 9 || rank >= 10 {
                        return None;
                    }
                    board.set(file, rank, piece as u8);
                    file += 1;
                }
                _ => return None,
            }
        }

        if rank != 9 || file != 9 {
            return None;
        }

        Some(board)
    }
}

#[cfg(test)]
mod tests {
    use super::{Board90, STARTPOS_FEN};

    #[test]
    fn startpos_roundtrip() {
        let board = Board90::from_fen(STARTPOS_FEN).expect("startpos fen");
        assert_eq!(board.to_fen(), STARTPOS_FEN);
    }
}
