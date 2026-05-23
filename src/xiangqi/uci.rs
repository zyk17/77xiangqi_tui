//! UCI/ICCS：列 `a-i`，行 `0-9`（红方底线为 `0`）。
//!
//! 全局唯一坐标：用户、引擎、轴标、历史、输入均为同一套（如炮二平五恒为 `h2e2`）。
//! 棋盘 `rotated` 仅翻转棋子显示与 [`screen_to_internal`] 命中，不改变 UCI 字符串。

pub fn parse_uci_coords(uci: &str) -> Option<(usize, usize, usize, usize)> {
    let b: Vec<char> = uci.chars().collect();
    if b.len() < 4 {
        return None;
    }
    let c1 = (b[0] as u8).checked_sub(b'a')? as usize;
    let r1 = 9usize.checked_sub(b[1].to_digit(10)? as usize)?;
    let c2 = (b[2] as u8).checked_sub(b'a')? as usize;
    let r2 = 9usize.checked_sub(b[3].to_digit(10)? as usize)?;
    if r1 < 10 && c1 < 9 && r2 < 10 && c2 < 9 {
        Some((r1, c1, r2, c2))
    } else {
        None
    }
}

pub fn uci_from_coords(r1: usize, c1: usize, r2: usize, c2: usize) -> String {
    format!(
        "{}{}{}{}",
        (b'a' + c1 as u8) as char,
        9 - r1,
        (b'a' + c2 as u8) as char,
        9 - r2
    )
}

/// 内部格在全局 UCI 下的格名（与轴标一致）。
#[inline]
pub fn uci_cell_label(file: u8, rank: u8) -> String {
    format!("{}{}", (b'a' + file) as char, 9 - rank)
}

/// 屏幕格 → 内部格（仅旋转显示/命中用）。
#[inline]
pub fn screen_to_internal(file: u8, screen_row: u8, rotated: bool) -> (u8, u8) {
    if rotated {
        (8 - file, 9 - screen_row)
    } else {
        (file, screen_row)
    }
}

#[inline]
pub fn internal_to_screen(file: u8, rank: u8, rotated: bool) -> (u8, u8) {
    if rotated {
        (8 - file, 9 - rank)
    } else {
        (file, rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h2e2_maps_to_internal_ranks() {
        let (r1, c1, r2, c2) = parse_uci_coords("h2e2").expect("coords");
        assert_eq!((r1, c1, r2, c2), (7, 7, 7, 4));
        assert_eq!(uci_from_coords(r1, c1, r2, c2), "h2e2");
    }

    #[test]
    fn uci_cell_label_is_global() {
        assert_eq!(uci_cell_label(0, 9), "a0");
        assert_eq!(uci_cell_label(7, 7), "h2");
        assert_eq!(uci_cell_label(8, 0), "i9");
    }

    #[test]
    fn screen_map_flips_when_rotated() {
        assert_eq!(screen_to_internal(0, 9, true), (8, 0));
        assert_eq!(uci_cell_label(8, 0), "i9");
    }
}
