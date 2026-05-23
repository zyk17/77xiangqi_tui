/// UCI/ICCS 坐标：列 `a-i`，行 `0-9`（`0` 为红方底线，与界面左侧标号一致）。
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

/// 界面左侧标号（0 在底线）→ 内部行索引（0 在顶线/FEN 首行）。
#[inline]
pub fn internal_rank_from_axis_label(label: u8) -> u8 {
    9 - label
}

#[inline]
pub fn axis_label_from_internal_rank(rank: u8) -> u8 {
    9 - rank
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
}
