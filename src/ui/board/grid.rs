//! 格子棋盘：10×9 方格，按终端区域自适应并在内区居中。
//! 棋子块占格 80%，汉字在棋子块内水平/垂直居中。
//! 轴标为全局 UCI（与引擎/输入一致）；`rotated` 仅翻转棋子与命中映射。

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthChar;

use crate::game::BoardArrow;
use crate::xiangqi::{Board90, screen_to_internal};

use super::pieces::{piece_cell_style, piece_label};
use super::super::style::{text as text_style, text_bold, text_dim};

const GRID_STROKE: Color = Color::Rgb(171, 93, 22);
/// 上一手 / 提示：红方高亮
const HIGHLIGHT_RED: Color = Color::Rgb(205, 125, 45);
const HIGHLIGHT_RED_PENDING: Color = Color::Rgb(230, 145, 55);
/// 上一手 / 提示：黑方高亮
const HIGHLIGHT_BLACK: Color = Color::Rgb(32, 58, 105);
const HIGHLIGHT_BLACK_PENDING: Color = Color::Rgb(45, 75, 135);
const AXIS_W: usize = 2;
/// 终端字符显示宽/高（约 8×16 px → 0.5），与 `scripts/check_board_aspect.py` 一致
pub(crate) const TERMINAL_CHAR_WH_RATIO: f64 = 0.5;
/// 棋盘外框目标宽高比：9 路 ÷ 10 线
pub(crate) const TARGET_BOARD_ASPECT: f64 = 9.0 / 10.0;
/// 棋子块占单元格显示区域的比例（宽、高各 80%）
const PIECE_FILL_PERCENT: usize = 80;
/// 画完 screen_row 4 后插河界
const RIVER_AFTER_SCREEN_ROW: u8 = 4;

#[derive(Clone, Copy, Debug)]
struct GridMetrics {
    cell_w: usize,
    cell_h: usize,
    piece_w: usize,
    piece_h: usize,
    piece_pad_w: usize,
    piece_pad_h: usize,
    glyph_sub: usize,
    pad_left: u16,
    pad_top: u16,
}

impl GridMetrics {
    fn from_area(inner: Rect) -> Self {
        let inner_w = inner.width.max(12) as usize;
        let inner_h = inner.height.max(12) as usize;

        let max_cell_w = ((inner_w.saturating_sub(AXIS_W + 1 + 8 + 1)) / 9).max(2);
        let max_cell_h = max_cell_h_for_inner_h(inner_h);

        let (cell_w, cell_h) = fit_board_cells(inner_w, inner_h, max_cell_w, max_cell_h);
        let (piece_w, piece_h, piece_pad_w, piece_pad_h, glyph_sub) =
            piece_layout_in_cell(cell_w, cell_h);

        let grid_cols = line_cols_for_cell_w(cell_w);
        let grid_lines = grid_line_count(cell_h);
        let pad_left = ((inner_w.saturating_sub(grid_cols)) / 2) as u16;
        let pad_top = ((inner_h.saturating_sub(grid_lines)) / 2) as u16;

        Self {
            cell_w,
            cell_h,
            piece_w,
            piece_h,
            piece_pad_w,
            piece_pad_h,
            glyph_sub,
            pad_left,
            pad_top,
        }
    }

    fn line_cols(&self) -> usize {
        line_cols_for_cell_w(self.cell_w)
    }
}

/// 对弈区棋盘在给定终端尺寸下拟合后的像素宽高比（与 `ui/mod.rs` 布局一致）。
pub fn battle_board_pixel_aspect(term_w: u16, term_h: u16) -> f64 {
    let board_area_h = term_h.saturating_sub(3 + 3 + 5) as usize;
    let inner_w = ((term_w as f32 * 0.72) as usize)
        .saturating_sub(2)
        .max(12);
    let inner_h = board_area_h.saturating_sub(2).max(12);
    let max_cell_w = ((inner_w.saturating_sub(AXIS_W + 1 + 8 + 1)) / 9).max(2);
    let max_cell_h = max_cell_h_for_inner_h(inner_h);
    let (cell_w, cell_h) = fit_board_cells(inner_w, inner_h, max_cell_w, max_cell_h);
    grid_pixel_aspect(cell_w, cell_h)
}

/// 棋盘外框在终端像素下的宽高比（列×字宽 / 行×字高）。
pub fn grid_pixel_aspect(cell_w: usize, cell_h: usize) -> f64 {
    let cols = line_cols_for_cell_w(cell_w) as f64;
    let lines = grid_line_count(cell_h) as f64;
    cols * TERMINAL_CHAR_WH_RATIO / lines
}

/// 在可用区域内选 `(cell_w, cell_h)`：优先接近 9:10 外框比，其次格子尽量大。
fn fit_board_cells(
    inner_w: usize,
    inner_h: usize,
    max_cell_w: usize,
    max_cell_h: usize,
) -> (usize, usize) {
    let mut best = (2usize, 1usize);
    let mut best_key = (u64::MAX, usize::MAX, 0usize); // (|aspect−target|, lines, −area)

    for cell_h in 1..=max_cell_h {
        for cell_w in 2..=max_cell_w {
            if line_cols_for_cell_w(cell_w) > inner_w || grid_line_count(cell_h) > inner_h {
                continue;
            }
            let err = (grid_pixel_aspect(cell_w, cell_h) - TARGET_BOARD_ASPECT).abs();
            let lines = grid_line_count(cell_h);
            let area = cell_w * cell_h;
            let key = (ordered_float(err), lines, usize::MAX - area);
            if key < best_key {
                best_key = key;
                best = (cell_w, cell_h);
            }
        }
    }
    best
}

#[inline]
fn ordered_float(x: f64) -> u64 {
    x.to_bits()
}

fn piece_layout_in_cell(cell_w: usize, cell_h: usize) -> (usize, usize, usize, usize, usize) {
    let piece_w = (cell_w * PIECE_FILL_PERCENT / 100).max(2).min(cell_w);
    let piece_h = (cell_h * PIECE_FILL_PERCENT / 100).max(1).min(cell_h);
    let piece_pad_w = (cell_w - piece_w) / 2;
    let piece_pad_h = (cell_h - piece_h) / 2;
    let glyph_sub = piece_pad_h + piece_h / 2;
    (
        piece_w,
        piece_h,
        piece_pad_w,
        piece_pad_h,
        glyph_sub,
    )
}

fn river_sep_lines(cell_h: usize) -> usize {
    cell_h + 2
}

fn grid_line_count(cell_h: usize) -> usize {
    1 + 10 * cell_h + 8 + river_sep_lines(cell_h) + 1 + 1
}

fn max_cell_h_for_inner_h(inner_h: usize) -> usize {
    let mut h = 1usize;
    while grid_line_count(h + 1) <= inner_h {
        h += 1;
    }
    h.max(1)
}

fn line_cols_for_cell_w(cell_w: usize) -> usize {
    AXIS_W + 1 + cell_w * 9 + 8 + 1
}

const GRID_MARKER_CHARS: &[char] = &[
    '┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼', '─', '│',
];

/// 从抓屏解析 `cell_w`/`cell_h` 并返回理论像素宽高比（与布局公式一致）。
pub fn measure_capture_grid_aspect(capture: &str) -> f64 {
    parse_capture_grid_cells(capture)
        .map(|(w, h)| grid_pixel_aspect(w, h))
        .unwrap_or(0.0)
}

/// 从 `┌──────┬…┐` 顶边与 `└──────┴…┘` 底边推算格子尺寸。
pub fn parse_capture_grid_cells(capture: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = capture.lines().collect();
    let top = lines.iter().position(|l| is_board_top_border(l))?;
    let mut bottom = lines.iter().rposition(|l| {
        l.contains('└') && l.chars().filter(|&c| c == '┴').count() >= 8
    })?;
    if bottom + 1 < lines.len() {
        let next = lines[bottom + 1];
        let file_labels = next.chars().filter(|c| matches!(c, 'a'..='i')).count();
        if file_labels >= 3 {
            bottom += 1;
        }
    }
    let cell_w = parse_cell_w_from_top_line(lines.get(top)?)?;
    let grid_lines = bottom - top + 1;
    for cell_h in 1..=8usize {
        let expected = grid_line_count(cell_h);
        if expected == grid_lines || expected == grid_lines + 1 {
            return Some((cell_w, cell_h));
        }
    }
    None
}

fn parse_cell_w_from_top_line(line: &str) -> Option<usize> {
    let i = line.find('┌')?;
    let tail = &line[i..];
    if tail.chars().filter(|&c| c == '┬').count() < 8 {
        return None;
    }
    let dashes = tail.chars().take_while(|&c| c == '─').count();
    if dashes >= 2 {
        Some(dashes)
    } else {
        None
    }
}

/// 是否为棋盘顶边 `┌────┬×8`（不要求行末 `┐`，避免抓屏裁切误判）。
fn is_board_top_border(line: &str) -> bool {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    for i in 0..chars.len() {
        if chars[i].1 != '┌' {
            continue;
        }
        let tail: String = chars[i..].iter().map(|(_, c)| *c).collect();
        if tail.chars().filter(|&c| c == '┬').count() >= 8 {
            return true;
        }
    }
    false
}

fn is_grid_marker(ch: char) -> bool {
    GRID_MARKER_CHARS.contains(&ch)
}

pub fn render_grid_board(
    frame: &mut Frame<'_>,
    area: Rect,
    board: &Board90,
    rotated: bool,
    last_arrow: Option<BoardArrow>,
    pending_arrow: Option<BoardArrow>,
    selected: Option<(u8, u8)>,
) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            if pending_arrow.is_some() {
                "A 棋盘 · 提示"
            } else {
                "A 棋盘"
            },
            text_bold(),
        ));
    let inner = board_block_inner(area);
    let m = grid_metrics_for_board_area(area);
    crate::runtime_log::debug(format!(
        "board_grid cell={}x{} piece={}x{} inner={:?} grid={}x{} pad=({},{})",
        m.cell_w,
        m.cell_h,
        m.piece_w,
        m.piece_h,
        inner,
        m.line_cols(),
        grid_line_count(m.cell_h),
        m.pad_left,
        m.pad_top
    ));
    let line_stroke = Style::default().fg(GRID_STROKE);
    let mut lines = Vec::new();
    lines.push(border_line(&m, '┌', '┬', '┐', line_stroke));
    for screen_row in 0..10_u8 {
        lines.extend(rank_block(
            &m,
            board,
            screen_row,
            rotated,
            last_arrow,
            pending_arrow,
            selected,
        ));
        if screen_row == 9 {
            break;
        }
        if screen_row == RIVER_AFTER_SCREEN_ROW {
            lines.extend(river_block(&m, line_stroke));
        } else {
            lines.push(border_line(&m, '├', '┼', '┤', line_stroke));
        }
    }
    lines.push(border_line(&m, '└', '┴', '┘', line_stroke));
    lines.push(file_axis_line(&m, rotated));

    let grid_w = m.line_cols() as u16;
    let grid_h = lines.len() as u16;
    let content = Rect {
        x: inner.x.saturating_add(m.pad_left),
        y: inner.y.saturating_add(m.pad_top),
        width: grid_w.min(inner.width),
        height: grid_h.min(inner.height.saturating_sub(m.pad_top)),
    };
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(lines)
            .style(text_style())
            .wrap(Wrap { trim: false }),
        content,
    );
    area
}

pub fn hit_board_cell(
    area: Rect,
    column: u16,
    row: u16,
    rotated: bool,
) -> Option<(u8, u8)> {
    let inner = board_block_inner(area);
    let m = grid_metrics_for_board_area(area);
    if column < inner.x
        || column >= inner.x + inner.width
        || row < inner.y
        || row >= inner.y + inner.height
    {
        return None;
    }
    let rel_col = column.saturating_sub(inner.x + m.pad_left);
    let rel_row = row.saturating_sub(inner.y + m.pad_top);
    let grid_w = m.line_cols() as u16;
    let grid_h = grid_line_count(m.cell_h) as u16;
    if rel_col >= grid_w || rel_row >= grid_h {
        return None;
    }
    let Some(screen_row) = screen_row_at_line(&m, rel_row) else {
        return None;
    };
    let file = file_at_column(&m, rel_col)?;
    Some(screen_to_internal(file, screen_row, rotated))
}

fn screen_row_at_line(m: &GridMetrics, rel_row: u16) -> Option<u8> {
    let ch = m.cell_h as u16;
    let mut line = 1u16;
    for screen_row in 0..10u8 {
        if rel_row >= line && rel_row < line + ch {
            return Some(screen_row);
        }
        line += ch;
        if screen_row == 9 {
            break;
        }
        if screen_row == RIVER_AFTER_SCREEN_ROW {
            line += river_sep_lines(m.cell_h) as u16;
        } else {
            line += 1;
        }
    }
    None
}

/// 按格宽 + 竖线列宽命中整格（点在分隔线上归左侧格）。
fn file_at_column(m: &GridMetrics, rel_col: u16) -> Option<u8> {
    const LEFT_EDGE: u16 = 1;
    let origin = AXIS_W as u16 + LEFT_EDGE;
    if rel_col < origin {
        return None;
    }
    let mut x = rel_col - origin;
    for file in 0..9u8 {
        let span = m.cell_w as u16;
        if x < span {
            return Some(file);
        }
        x = x.saturating_sub(span);
        if file == 8 {
            break;
        }
        if x == 0 {
            return Some(file);
        }
        x = x.saturating_sub(1);
    }
    None
}

fn board_block_inner(area: Rect) -> Rect {
    Block::default().borders(Borders::ALL).inner(area)
}

fn grid_metrics_for_board_area(area: Rect) -> GridMetrics {
    GridMetrics::from_area(board_block_inner(area))
}

/// 屏幕格中心在终端上的坐标（用于命中回归测试）。
pub fn cell_hit_point_in_grid(area: Rect, file: u8, screen_row: u8) -> Option<(u16, u16)> {
    let inner = board_block_inner(area);
    let m = grid_metrics_for_board_area(area);
    let rel_col = column_center_for_file(&m, file)?;
    let rel_row = line_center_for_screen_row(&m, screen_row)?;
    Some((
        inner.x.saturating_add(m.pad_left).saturating_add(rel_col),
        inner.y.saturating_add(m.pad_top).saturating_add(rel_row),
    ))
}

fn column_center_for_file(m: &GridMetrics, file: u8) -> Option<u16> {
    let mut x = (AXIS_W + 1) as u16;
    for f in 0..9u8 {
        if f == file {
            return Some(x + (m.cell_w as u16) / 2);
        }
        x = x.saturating_add(m.cell_w as u16 + 1);
    }
    None
}

fn line_center_for_screen_row(m: &GridMetrics, screen_row: u8) -> Option<u16> {
    let ch = m.cell_h as u16;
    let mut line = 1u16;
    for sr in 0..10u8 {
        if sr == screen_row {
            return Some(line + ch / 2);
        }
        line += ch;
        if sr == 9 {
            break;
        }
        if sr == RIVER_AFTER_SCREEN_ROW {
            line += river_sep_lines(m.cell_h) as u16;
        } else {
            line += 1;
        }
    }
    None
}

fn rank_block(
    m: &GridMetrics,
    board: &Board90,
    screen_row: u8,
    rotated: bool,
    last: Option<BoardArrow>,
    pending: Option<BoardArrow>,
    selected: Option<(u8, u8)>,
) -> Vec<Line<'static>> {
    let (_, irank) = screen_to_internal(0, screen_row, rotated);
    let axis = 9 - irank;
    (0..m.cell_h)
        .map(|sub| {
            let mut spans = vec![Span::styled(
                if sub == 0 {
                    format!("{axis:>AXIS_W$}")
                } else {
                    " ".repeat(AXIS_W)
                },
                text_dim(),
            )];
            spans.push(Span::styled("│", Style::default().fg(GRID_STROKE)));
            for file in 0..9u8 {
                let (ifile, irank) = screen_to_internal(file, screen_row, rotated);
                let piece = board.get(ifile, irank);
                spans.extend(cell_spans(
                    m,
                    board,
                    piece,
                    ifile,
                    irank,
                    sub,
                    last,
                    pending,
                    selected,
                ));
                if file != 8 {
                    spans.push(Span::styled("│", Style::default().fg(GRID_STROKE)));
                }
            }
            spans.push(Span::styled("│", Style::default().fg(GRID_STROKE)));
            Line::from(spans)
        })
        .collect()
}

fn cell_spans(
    m: &GridMetrics,
    board: &Board90,
    cell: u8,
    file: u8,
    rank: u8,
    sub: usize,
    last: Option<BoardArrow>,
    pending: Option<BoardArrow>,
    selected: Option<(u8, u8)>,
) -> Vec<Span<'static>> {
    let has_piece = piece_label(cell).is_some();
    if !has_piece {
        let style = empty_cell_style(board, file, rank, last, pending, selected);
        return vec![Span::styled(fit_display("", m.cell_w), style)];
    }

    let in_piece_band =
        sub >= m.piece_pad_h && sub < m.piece_pad_h.saturating_add(m.piece_h);
    if !in_piece_band {
        return vec![Span::styled(fit_display("", m.cell_w), text_dim())];
    }

    let show_glyph = sub == m.glyph_sub;
    let base = piece_label(cell)
        .map(|(_, red)| piece_cell_style(red))
        .unwrap_or(text_dim());
    let style = apply_highlights(base, board, file, rank, last, pending, selected);

    let right_w = m.cell_w.saturating_sub(m.piece_pad_w + m.piece_w);
    let mut spans = Vec::with_capacity(3);
    if m.piece_pad_w > 0 {
        spans.push(Span::raw(" ".repeat(m.piece_pad_w)));
    }
    let piece_text = if show_glyph {
        piece_label(cell)
            .map(|(label, _)| fit_display(label, m.piece_w))
            .unwrap_or_else(|| fit_display("", m.piece_w))
    } else {
        fit_display("", m.piece_w)
    };
    spans.push(Span::styled(piece_text, style));
    if right_w > 0 {
        spans.push(Span::raw(" ".repeat(right_w)));
    }
    spans
}

fn fit_display(text: &str, width: usize) -> String {
    let w: usize = text
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
        .sum();
    if w >= width {
        return text.to_string();
    }
    let pad = width - w;
    let left = pad / 2;
    format!("{}{}{}", " ".repeat(left), text, " ".repeat(pad - left))
}

fn arrow_mover_is_red(board: &Board90, arrow: &BoardArrow) -> bool {
    if !board.is_empty(arrow.to_file, arrow.to_rank) {
        board.is_red_piece(arrow.to_file, arrow.to_rank)
    } else if !board.is_empty(arrow.from_file, arrow.from_rank) {
        board.is_red_piece(arrow.from_file, arrow.from_rank)
    } else {
        true
    }
}

fn side_highlight_bg(red: bool, pending: bool) -> Color {
    if red {
        if pending {
            HIGHLIGHT_RED_PENDING
        } else {
            HIGHLIGHT_RED
        }
    } else if pending {
        HIGHLIGHT_BLACK_PENDING
    } else {
        HIGHLIGHT_BLACK
    }
}

fn move_highlight_overlay(
    board: &Board90,
    file: u8,
    rank: u8,
    last: Option<BoardArrow>,
    pending: Option<BoardArrow>,
) -> Option<(Color, bool)> {
    if let Some(a) = pending {
        if (a.from_file == file && a.from_rank == rank)
            || (a.to_file == file && a.to_rank == rank)
        {
            let red = arrow_mover_is_red(board, &a);
            return Some((side_highlight_bg(red, true), true));
        }
    }
    if let Some(a) = last {
        if (a.from_file == file && a.from_rank == rank)
            || (a.to_file == file && a.to_rank == rank)
        {
            let red = arrow_mover_is_red(board, &a);
            return Some((side_highlight_bg(red, false), false));
        }
    }
    None
}

fn apply_highlights(
    mut style: Style,
    board: &Board90,
    file: u8,
    rank: u8,
    last: Option<BoardArrow>,
    pending: Option<BoardArrow>,
    selected: Option<(u8, u8)>,
) -> Style {
    if selected == Some((file, rank)) {
        style = style.bg(Color::DarkGray);
    }
    if let Some((bg, bold)) = move_highlight_overlay(board, file, rank, last, pending) {
        style = style.bg(bg);
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
    }
    style
}

fn empty_cell_style(
    board: &Board90,
    file: u8,
    rank: u8,
    last: Option<BoardArrow>,
    pending: Option<BoardArrow>,
    selected: Option<(u8, u8)>,
) -> Style {
    if let Some((bg, bold)) = move_highlight_overlay(board, file, rank, last, pending) {
        let mut s = Style::default().bg(bg);
        if bold {
            s = s.add_modifier(Modifier::BOLD);
        }
        return s;
    }
    if selected == Some((file, rank)) {
        return Style::default().bg(Color::DarkGray);
    }
    text_dim()
}

fn border_line(
    m: &GridMetrics,
    left: char,
    mid: char,
    right: char,
    stroke: Style,
) -> Line<'static> {
    let mut spans = vec![Span::raw(" ".repeat(AXIS_W))];
    spans.push(Span::styled(left.to_string(), stroke));
    for file in 0..9u8 {
        spans.push(Span::styled("─".repeat(m.cell_w), stroke));
        spans.push(Span::styled(
            if file == 8 { right } else { mid }.to_string(),
            stroke,
        ));
    }
    Line::from(spans)
}

fn river_block(m: &GridMetrics, stroke: Style) -> Vec<Line<'static>> {
    let inner_w = river_inner_w(m.cell_w);
    let text_row = m.glyph_sub;
    let mut lines = vec![border_line(m, '├', '┴', '┤', stroke)];
    for sub in 0..m.cell_h {
        let inner = if sub == text_row {
            pad_river("楚河 77象棋 漢界", inner_w)
        } else {
            fit_display("", inner_w)
        };
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(AXIS_W)),
            Span::styled("│", stroke),
            Span::styled(
                inner,
                stroke.add_modifier(if sub == text_row {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Span::styled("│", stroke),
        ]));
    }
    lines.push(border_line(m, '├', '┬', '┤', stroke));
    lines
}

fn pad_river(text: &str, width: usize) -> String {
    fit_display(text, width)
}

fn river_inner_w(cell_w: usize) -> usize {
    cell_w * 9 + 8
}

fn file_axis_line(m: &GridMetrics, rotated: bool) -> Line<'static> {
    let stroke = Style::default().fg(GRID_STROKE);
    let mut spans = vec![Span::raw(" ".repeat(AXIS_W)), Span::styled(" ", stroke)];
    for file in 0..9u8 {
        let (ifile, _) = screen_to_internal(file, 9, rotated);
        let ch = (b'a' + ifile) as char;
        spans.push(Span::styled(fit_display(&ch.to_string(), m.cell_w), text_dim()));
        if file != 8 {
            spans.push(Span::styled("│", stroke));
        }
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_label_is_global_uci() {
        let (_, r9) = screen_to_internal(0, 9, false);
        assert_eq!(9 - r9, 0);
        let (_, r0) = screen_to_internal(0, 0, true);
        assert_eq!(9 - r0, 0);
    }

    #[test]
    fn river_after_rank_four() {
        assert_eq!(RIVER_AFTER_SCREEN_ROW, 4);
    }

    #[test]
    fn grid_line_count_matches_render_for_cell_h_two() {
        assert_eq!(grid_line_count(2), 1 + 20 + 8 + 4 + 2);
    }

    #[test]
    fn fit_board_cells_near_target_aspect() {
        let inner_w = 84usize;
        let inner_h = 43usize;
        let max_cell_w = (inner_w - 12) / 9;
        let max_cell_h = max_cell_h_for_inner_h(inner_h);
        let (w, h) = fit_board_cells(inner_w, inner_h, max_cell_w, max_cell_h);
        let aspect = grid_pixel_aspect(w, h);
        assert!(
            (aspect - TARGET_BOARD_ASPECT).abs() < 0.12,
            "aspect {aspect:.3} for {w}x{h}"
        );
        assert!(line_cols_for_cell_w(w) <= inner_w);
        assert!(grid_line_count(h) <= inner_h);
    }

    #[test]
    fn capture_like_inner_prefers_wider_cells() {
        let (w, h) = fit_board_cells(84, 43, 8, 2);
        assert!(w >= 5, "got {w}x{h}");
        assert!((grid_pixel_aspect(w, h) - TARGET_BOARD_ASPECT).abs() < 0.1);
    }

    #[test]
    fn piece_inset_is_eighty_percent() {
        let (pw, ph, _, _, _) = piece_layout_in_cell(10, 5);
        assert_eq!(pw, 8);
        assert_eq!(ph, 4);
    }

    #[test]
    fn pad_river_matches_grid_inner_width() {
        let cell_w = 6usize;
        let inner = river_inner_w(cell_w);
        let s = pad_river("楚河 77象棋 漢界", inner);
        let w: usize = s
            .chars()
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
            .sum();
        assert_eq!(w, inner);
    }

    #[test]
    fn fit_display_centers_cjk() {
        assert_eq!(fit_display("車", 4), " 車 ");
    }

    #[test]
    fn bottom_left_screen_maps_to_global_uci_label() {
        use crate::xiangqi::uci_cell_label;

        assert_eq!(screen_to_internal(0, 9, false), (0, 9));
        assert_eq!(uci_cell_label(0, 9), "a0");
        let (f, r) = screen_to_internal(0, 9, true);
        assert_eq!(uci_cell_label(f, r), "i9");
    }

    #[test]
    fn file_at_column_maps_whole_cell_band() {
        let m = GridMetrics {
            cell_w: 6,
            cell_h: 2,
            piece_w: 4,
            piece_h: 1,
            piece_pad_w: 1,
            piece_pad_h: 0,
            glyph_sub: 0,
            pad_left: 0,
            pad_top: 0,
        };
        let origin = (AXIS_W + 1) as u16;
        assert_eq!(file_at_column(&m, origin), Some(0));
        assert_eq!(file_at_column(&m, origin + 3), Some(0));
        assert_eq!(file_at_column(&m, origin + 6), Some(0));
        assert_eq!(file_at_column(&m, origin + 7), Some(1));
        assert_eq!(file_at_column(&m, origin + 13), Some(1));
    }

    #[test]
    fn screen_rotate_maps_corners() {
        use crate::xiangqi::internal_to_screen;

        assert_eq!(screen_to_internal(0, 9, false), (0, 9));
        assert_eq!(screen_to_internal(0, 9, true), (8, 0));
        assert_eq!(internal_to_screen(8, 0, true), (0, 9));
        assert_eq!(internal_to_screen(0, 9, false), (0, 9));
    }

    #[test]
    fn piece_glyph_row_inside_piece_band() {
        let (_, _, _, pad_h, glyph) = piece_layout_in_cell(6, 4);
        assert!(glyph >= pad_h);
        assert!(glyph < pad_h + 4);
    }

    #[test]
    fn battle_viewport_aspect_near_nine_tenths() {
        use crate::ui::board::capture::CAPTURE_HEIGHT;

        let aspect = battle_board_pixel_aspect(120, CAPTURE_HEIGHT);
        assert!(
            (aspect - TARGET_BOARD_ASPECT).abs() < 0.12,
            "aspect {aspect:.3}"
        );
    }
}
