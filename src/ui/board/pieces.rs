//! 棋子汉字与格内样式。

use ratatui::style::{Color, Modifier, Style};

#[inline]
pub fn piece_label(cell: u8) -> Option<(&'static str, bool)> {
    if cell == 0 {
        return None;
    }
    let (kind, red) = piece_kind_side(cell)?;
    let label = match kind {
        5 => {
            if red {
                "帥"
            } else {
                "將"
            }
        }
        4 => {
            if red {
                "仕"
            } else {
                "士"
            }
        }
        3 => {
            if red {
                "相"
            } else {
                "象"
            }
        }
        2 => {
            if red {
                "傌"
            } else {
                "馬"
            }
        }
        1 => {
            if red {
                "俥"
            } else {
                "車"
            }
        }
        6 => {
            if red {
                "炮"
            } else {
                "砲"
            }
        }
        7 => {
            if red {
                "兵"
            } else {
                "卒"
            }
        }
        _ => return None,
    };
    Some((label, red))
}

fn piece_kind_side(cell: u8) -> Option<(u8, bool)> {
    if cell == 0 {
        None
    } else if (1..=7).contains(&cell) {
        Some((cell, true))
    } else if (9..=15).contains(&cell) {
        Some((cell - 8, false))
    } else {
        None
    }
}

pub fn piece_cell_style(red: bool) -> Style {
    if red {
        Style::default()
            .fg(Color::White)
            .bg(Color::Rgb(188, 51, 40))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .bg(Color::Rgb(52, 52, 52))
            .add_modifier(Modifier::BOLD)
    }
}
