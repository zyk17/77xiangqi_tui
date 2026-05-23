//! 棋盘唯一表示：`Board90` 的 `cells` 为紧凑编码（`0` 空；红 `1..=7`；黑 `9..=15`）。

use super::side::Side;
use super::uci::{parse_uci_coords, uci_from_coords};

pub const STARTPOS_FEN: &str =
    "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

const EMPTY: u8 = 0;

type KingPositions = (Option<(i32, i32)>, Option<(i32, i32)>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board90 {
    pub cells: [u8; 90],
}

impl Default for Board90 {
    fn default() -> Self {
        Self { cells: [EMPTY; 90] }
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
    fn at(&self, rank: usize, file: usize) -> u8 {
        self.cells[rank * 9 + file]
    }

    #[inline]
    fn set_at(&mut self, rank: usize, file: usize, piece: u8) {
        self.cells[rank * 9 + file] = piece;
    }

    pub fn is_empty(&self, file: u8, rank: u8) -> bool {
        self.get(file, rank) == EMPTY
    }

    pub fn is_red_piece(&self, file: u8, rank: u8) -> bool {
        is_red_piece(self.get(file, rank))
    }

    /// 该格棋子所属方；空格为 `None`。
    pub fn piece_side(&self, file: u8, rank: u8) -> Option<Side> {
        let cell = self.get(file, rank);
        if cell == EMPTY {
            return None;
        }
        if is_red_piece(cell) {
            Some(Side::Red)
        } else {
            Some(Side::Black)
        }
    }

    pub fn is_own_for(&self, file: u8, rank: u8, side: Side) -> bool {
        self.piece_side(file, rank) == Some(side)
    }

    pub fn from_fen(fen: &str) -> Option<Self> {
        Self::from_fen_with_side(fen).map(|(b, _)| b)
    }

    pub fn from_fen_with_side(fen: &str) -> Option<(Self, Side)> {
        let fen = fen.trim();
        if fen.is_empty() {
            return None;
        }
        let parts: Vec<&str> = fen.split_whitespace().collect();
        let board_part = parts.first().copied()?;
        let side = if parts.len() < 2 {
            Side::Red
        } else {
            Side::from_fen_turn_field(parts[1])
        };
        let rows: Vec<&str> = board_part.split('/').collect();
        if rows.len() != 10 {
            return None;
        }
        let mut cells = [EMPTY; 90];
        for (r, row_text) in rows.iter().enumerate() {
            let mut c = 0usize;
            for ch in row_text.chars() {
                if ch.is_ascii_digit() {
                    let n = ch.to_digit(10)? as usize;
                    if n == 0 || c + n > 9 {
                        return None;
                    }
                    c += n;
                    continue;
                }
                if c >= 9 {
                    return None;
                }
                let is_red = ch.is_uppercase();
                let kind = match ch.to_ascii_uppercase() {
                    'R' => 1u8,
                    'N' => 2u8,
                    'B' => 3u8,
                    'A' => 4u8,
                    'K' => 5u8,
                    'C' => 6u8,
                    'P' => 7u8,
                    _ => return None,
                };
                cells[r * 9 + c] = if is_red { kind } else { kind + 8 };
                c += 1;
            }
            if c != 9 {
                return None;
            }
        }
        Some((Self { cells }, side))
    }

    pub fn to_fen(&self, side: Side) -> String {
        let mut rows = Vec::with_capacity(10);
        for r in 0..10usize {
            let mut row = String::new();
            let mut empty_run = 0u8;
            for f in 0..9usize {
                let cell = self.at(r, f);
                if cell == EMPTY {
                    empty_run += 1;
                    continue;
                }
                if empty_run > 0 {
                    row.push(char::from(b'0' + empty_run));
                    empty_run = 0;
                }
                row.push(piece_to_fen_char(cell));
            }
            if empty_run > 0 {
                row.push(char::from(b'0' + empty_run));
            }
            rows.push(row);
        }
        format!("{} {} - - 0 1", rows.join("/"), side.fen_turn_char())
    }

    pub fn apply_uci(&mut self, uci: &str) -> bool {
        let Some((r1, c1, r2, c2)) = parse_uci_coords(uci) else {
            return false;
        };
        let p = self.at(r1, c1);
        self.set_at(r1, c1, EMPTY);
        self.set_at(r2, c2, p);
        true
    }

    pub fn legal_ucis_for_side(&self, side: Side) -> Vec<String> {
        let side_red = side.is_red();
        let pseudo = self.pseudo_legal_ucis(side_red);
        let mut out = Vec::new();
        for uci in pseudo {
            let mut scratch = self.clone();
            if !scratch.apply_uci(&uci) {
                continue;
            }
            if scratch.kings_face_each_other() {
                continue;
            }
            if scratch.side_in_check(side_red) {
                continue;
            }
            out.push(uci);
        }
        out.sort();
        out.dedup();
        out
    }

    fn pseudo_legal_ucis(&self, side_red: bool) -> Vec<String> {
        let mut out = Vec::new();
        for r1 in 0i32..10 {
            for c1 in 0i32..9 {
                if !is_side(self.at(r1 as usize, c1 as usize), side_red) {
                    continue;
                }
                for r2 in 0i32..10 {
                    for c2 in 0i32..9 {
                        if r1 == r2 && c1 == c2 {
                            continue;
                        }
                        if self.legal_move_geometry(side_red, r1, c1, r2, c2) {
                            out.push(uci_from_coords(
                                r1 as usize,
                                c1 as usize,
                                r2 as usize,
                                c2 as usize,
                            ));
                        }
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn legal_move_geometry(&self, side_red: bool, r1: i32, c1: i32, r2: i32, c2: i32) -> bool {
        if !in_board(r2, c2) {
            return false;
        }
        if is_side(self.at(r2 as usize, c2 as usize), side_red) {
            return false;
        }
        let src = self.at(r1 as usize, c1 as usize);
        if src == EMPTY {
            return false;
        }
        let t = piece_kind_to_type(piece_kind(src));
        let Some(t) = t else {
            return false;
        };
        legal_move_geometry_raw(self, side_red, t, r1, c1, r2, c2)
    }

    fn kings_face_each_other(&self) -> bool {
        let (Some((r1, c1)), Some((r2, c2))) = self.find_king_positions() else {
            return false;
        };
        if c1 != c2 {
            return false;
        }
        count_between(self, r1, c1, r2, c2) == 0
    }

    fn side_in_check(&self, side_red: bool) -> bool {
        let king_code = if side_red { 5u8 } else { 13u8 };
        let mut kr = None;
        for r in 0..10usize {
            for c in 0..9usize {
                if self.at(r, c) == king_code {
                    kr = Some((r as i32, c as i32));
                    break;
                }
            }
        }
        let Some((kr, kc)) = kr else {
            return false;
        };
        self.is_square_attacked(kr, kc, !side_red)
    }

    fn is_square_attacked(&self, tr: i32, tc: i32, attacker_red: bool) -> bool {
        if !in_board(tr, tc) {
            return false;
        }
        for r1 in 0i32..10 {
            for c1 in 0i32..9 {
                if !is_side(self.at(r1 as usize, c1 as usize), attacker_red) {
                    continue;
                }
                if self.legal_move_geometry(attacker_red, r1, c1, tr, tc) {
                    return true;
                }
            }
        }
        false
    }

    fn find_king_positions(&self) -> KingPositions {
        let mut red = None;
        let mut black = None;
        for r in 0..10usize {
            for c in 0..9usize {
                match self.at(r, c) {
                    5 => red = Some((r as i32, c as i32)),
                    13 => black = Some((r as i32, c as i32)),
                    _ => {}
                }
            }
        }
        (red, black)
    }
}

#[inline]
fn is_red_piece(cell: u8) -> bool {
    (1..=7).contains(&cell)
}

#[inline]
fn is_side(cell: u8, side_red: bool) -> bool {
    if cell == EMPTY {
        return false;
    }
    if side_red {
        is_red_piece(cell)
    } else {
        (9..=15).contains(&cell)
    }
}

#[inline]
fn piece_kind(cell: u8) -> u8 {
    if cell == EMPTY {
        return 0;
    }
    if cell <= 7 { cell } else { cell - 8 }
}

fn piece_kind_to_type(kind: u8) -> Option<u8> {
    match kind {
        1 => Some(b'R'),
        2 => Some(b'N'),
        3 => Some(b'B'),
        4 => Some(b'A'),
        5 => Some(b'K'),
        6 => Some(b'C'),
        7 => Some(b'P'),
        _ => None,
    }
}

fn piece_to_fen_char(cell: u8) -> char {
    if cell == EMPTY {
        return '.';
    }
    let kind = piece_kind(cell);
    let ch = match kind {
        1 => 'R',
        2 => 'N',
        3 => 'B',
        4 => 'A',
        5 => 'K',
        6 => 'C',
        7 => 'P',
        _ => '.',
    };
    if is_red_piece(cell) {
        ch
    } else {
        ch.to_ascii_lowercase()
    }
}

#[inline]
fn in_board(r: i32, c: i32) -> bool {
    (0..10).contains(&r) && (0..9).contains(&c)
}

fn count_between(board: &Board90, r1: i32, c1: i32, r2: i32, c2: i32) -> i32 {
    let mut n = 0;
    if r1 == r2 {
        let (a, b) = if c1 < c2 { (c1, c2) } else { (c2, c1) };
        for c in (a + 1)..b {
            if board.at(r1 as usize, c as usize) != EMPTY {
                n += 1;
            }
        }
    } else if c1 == c2 {
        let (a, b) = if r1 < r2 { (r1, r2) } else { (r2, r1) };
        for r in (a + 1)..b {
            if board.at(r as usize, c1 as usize) != EMPTY {
                n += 1;
            }
        }
    }
    n
}

fn legal_move_geometry_raw(
    board: &Board90,
    side_red: bool,
    t: u8,
    r1: i32,
    c1: i32,
    r2: i32,
    c2: i32,
) -> bool {
    let dr = r2 - r1;
    let dc = c2 - c1;
    let adr = dr.abs();
    let adc = dc.abs();
    match t {
        b'R' => (r1 == r2 || c1 == c2) && count_between(board, r1, c1, r2, c2) == 0,
        b'C' => {
            if !(r1 == r2 || c1 == c2) {
                return false;
            }
            let between = count_between(board, r1, c1, r2, c2);
            let target_empty = board.at(r2 as usize, c2 as usize) == EMPTY;
            (between == 0 && target_empty) || (between == 1 && !target_empty)
        }
        b'N' => {
            if !((adr == 2 && adc == 1) || (adr == 1 && adc == 2)) {
                return false;
            }
            let (leg_r, leg_c) = if adr == 2 {
                (r1 + dr.signum(), c1)
            } else {
                (r1, c1 + dc.signum())
            };
            board.at(leg_r as usize, leg_c as usize) == EMPTY
        }
        b'B' => {
            if !(adr == 2 && adc == 2) {
                return false;
            }
            let eye_r = r1 + dr / 2;
            let eye_c = c1 + dc / 2;
            if board.at(eye_r as usize, eye_c as usize) != EMPTY {
                return false;
            }
            if side_red && r2 < 5 {
                return false;
            }
            if !side_red && r2 > 4 {
                return false;
            }
            true
        }
        b'A' => {
            if !(adr == 1 && adc == 1) {
                return false;
            }
            if !(3..=5).contains(&c2) {
                return false;
            }
            if side_red {
                (7..=9).contains(&r2)
            } else {
                (0..=2).contains(&r2)
            }
        }
        b'K' => {
            if !((adr == 1 && adc == 0) || (adr == 0 && adc == 1)) {
                return false;
            }
            if !(3..=5).contains(&c2) {
                return false;
            }
            if side_red {
                (7..=9).contains(&r2)
            } else {
                (0..=2).contains(&r2)
            }
        }
        b'P' => {
            let fwd = if side_red { -1 } else { 1 };
            let crossed = if side_red { r1 <= 4 } else { r1 >= 5 };
            (dr == fwd && dc == 0) || (crossed && dr == 0 && adc == 1)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Board90, STARTPOS_FEN, Side};

    #[test]
    fn startpos_roundtrip() {
        let board = Board90::from_fen(STARTPOS_FEN).expect("startpos fen");
        assert_eq!(board.to_fen(Side::Red), STARTPOS_FEN);
    }

    #[test]
    fn invalid_fen_rejected() {
        assert!(Board90::from_fen("rnbakabnr/9").is_none());
        assert!(Board90::from_fen("").is_none());
    }

    #[test]
    fn h2e2_legal_from_start() {
        let board = Board90::from_fen(STARTPOS_FEN).expect("start");
        assert!(
            board
                .legal_ucis_for_side(Side::Red)
                .contains(&"h2e2".to_string())
        );
    }
}
