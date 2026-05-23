//! 每帧渲染产出的可点击区域；命中检测只读这些 Rect，禁止硬编码终端行列。

use ratatui::layout::Rect;

use crate::app::{settings_field::SettingsField, BattleButton};

/// 顶栏两个 Tab 的命中区（与 `render_tabs` 的 50/50 分栏一致）。
#[derive(Debug, Clone, Copy)]
pub struct TabRegions {
    pub battle: Rect,
    pub settings: Rect,
}

#[derive(Debug, Clone, Copy)]
pub struct BattleRegions {
    pub board: Rect,
    pub board_rotated: bool,
    pub command_input: Rect,
    /// 与 `BUTTON_ROWS` 顺序一致，仅包含 `Some(button)` 的格。
    pub buttons: [(BattleButton, Rect); 11],
    pub button_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SettingsRegions {
    pub fields: [(SettingsField, Rect); SettingsField::ALL.len()],
    pub field_count: usize,
    pub command_input: Rect,
}

#[derive(Debug, Clone, Copy)]
pub enum ScreenRegions {
    Battle(BattleRegions),
    Settings(SettingsRegions),
}

/// 当前帧全部 UI 命中信息，由 `ui::render` 填充。
#[derive(Debug, Clone, Copy)]
pub struct UiRegions {
    pub tabs: TabRegions,
    pub screen: ScreenRegions,
}

impl UiRegions {
    pub fn battle(&self) -> Option<BattleRegions> {
        match self.screen {
            ScreenRegions::Battle(b) => Some(b),
            ScreenRegions::Settings(_) => None,
        }
    }

    pub fn settings(&self) -> Option<SettingsRegions> {
        match self.screen {
            ScreenRegions::Settings(s) => Some(s),
            ScreenRegions::Battle(_) => None,
        }
    }
}

pub fn point_in(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.right()
        && row >= rect.y
        && row < rect.bottom()
}
