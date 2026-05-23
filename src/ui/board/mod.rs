//! 格子棋盘。

mod capture;
mod grid;
mod pieces;

pub use capture::{capture_frame_text, write_capture};
pub use grid::{cell_hit_point_in_grid, hit_board_cell, render_grid_board};
