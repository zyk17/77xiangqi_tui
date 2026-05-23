//! 鼠标命中：仅依赖 `UiRegions` + 当前 `Screen`，对弈/设置互不串区。

use crate::app::{Screen, TopTab};

use super::HitTarget;
use super::layout::{BattleRegions, SettingsRegions, UiRegions, point_in};

pub fn hit_test(column: u16, row: u16, screen: Screen, regions: &UiRegions) -> Option<HitTarget> {
    if point_in(regions.tabs.battle, column, row) {
        return Some(HitTarget::TopTab(TopTab::Battle));
    }
    if point_in(regions.tabs.settings, column, row) {
        return Some(HitTarget::TopTab(TopTab::Settings));
    }

    match screen {
        Screen::Battle => regions
            .battle()
            .and_then(|battle| hit_battle(column, row, battle)),
        Screen::Settings => regions
            .settings()
            .and_then(|settings| hit_settings(column, row, settings)),
    }
}

fn hit_battle(column: u16, row: u16, battle: BattleRegions) -> Option<HitTarget> {
    if point_in(battle.command_input, column, row) {
        return Some(HitTarget::CommandInput);
    }
    for i in 0..battle.button_count {
        let (button, rect) = battle.buttons[i];
        if point_in(rect, column, row) {
            return Some(HitTarget::BattleButton(button));
        }
    }
    super::board::hit_board_cell(battle.board, column, row, battle.board_rotated)
        .map(|(file, rank)| HitTarget::BoardCell(file, rank))
}

fn hit_settings(column: u16, row: u16, settings: SettingsRegions) -> Option<HitTarget> {
    if point_in(settings.command_input, column, row) {
        return Some(HitTarget::CommandInput);
    }
    for i in 0..settings.field_count {
        let (field, rect) = settings.fields[i];
        if point_in(rect, column, row) {
            return Some(HitTarget::SettingsField(field));
        }
    }
    None
}
