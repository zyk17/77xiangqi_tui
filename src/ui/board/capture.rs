//! 无交互终端帧捕获，供调试与 CI 快照。

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::app::App;
use crate::game::BoardArrow;
use crate::ui;

pub fn capture_frame_text(width: u16, height: u16, app: &App) -> String {
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).expect("test terminal");
    term.draw(|f| {
        ui::render(f, app);
    })
    .expect("draw");
    let buf = term.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 抓屏高度：40 行棋盘 + 顶栏/输入/状态。
pub const CAPTURE_HEIGHT: u16 = 56;

pub fn write_capture(path: &std::path::Path, app: &App) {
    let text = capture_frame_text(120, CAPTURE_HEIGHT, app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, text);
}

pub fn app_with_pending_arrow() -> App {
    let mut app = App::default();
    app.game.last_move_arrow = Some(BoardArrow {
        from_file: 7,
        from_rank: 7,
        to_file: 4,
        to_rank: 7,
    });
    app.game.pending_arrow = Some(BoardArrow {
        from_file: 7,
        from_rank: 7,
        to_file: 4,
        to_rank: 7,
    });
    app.game.analysis.best_move = "h2e2".to_string();
    app
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::ui::board::grid::{self, TARGET_BOARD_ASPECT};

    #[test]
    fn capture_startpos_writes_board_snapshot() {
        let app = App::default();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("logs/board_capture.txt");
        write_capture(&path, &app);
        let text = std::fs::read_to_string(&path).expect("read capture");
        assert!(
            text.contains("77象棋") || text.contains('河'),
            "river text: {text}"
        );
        assert!(
            text.contains('車') || text.contains('帥') || text.contains('將'),
            "pieces: {text}"
        );
        assert!(
            text.contains('車') && text.contains('俥'),
            "both rooks visible: {text}"
        );
        assert!(
            text.contains('將') && text.contains('帥'),
            "both kings visible: {text}"
        );
        assert!(
            text.contains('a') && text.contains('i') && text.contains('└'),
            "bottom file axis a–i: {text}"
        );
        assert!(
            text.contains('┴') && text.contains('┬'),
            "river joins ranks 4/5: {text}"
        );
        let aspect = grid::battle_board_pixel_aspect(120, CAPTURE_HEIGHT);
        assert!(
            (aspect - TARGET_BOARD_ASPECT).abs() < 0.12,
            "fitted board aspect {aspect:.3}, want {TARGET_BOARD_ASPECT:.3}"
        );
        if let Some((w, h)) = grid::parse_capture_grid_cells(&text) {
            assert!((grid::grid_pixel_aspect(w, h) - aspect).abs() < 0.05);
        }
    }

    #[test]
    fn capture_shows_pending_move_hint() {
        let app = app_with_pending_arrow();
        let text = capture_frame_text(120, CAPTURE_HEIGHT, &app);
        assert!(
            text.contains("提示") || text.contains('炮') || text.contains('兵'),
            "pending move UI: {text}"
        );
    }
}
