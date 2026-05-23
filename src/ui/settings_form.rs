use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{settings_field::pick_mode_label, App, SettingsField};
use super::style::{
    border_focused, border_normal, highlight, text as text_style, text_bold,
};
use super::{display_or_placeholder, yes_no};

const ROW_HEIGHT: u16 = 1;

fn label_column_width() -> usize {
    SettingsField::ALL
        .iter()
        .map(|field| field.label().width())
        .max()
        .unwrap_or(8)
        .max(8)
}

fn pad_label(label: &str, width: usize) -> String {
    let w = label.width();
    if w >= width {
        label.to_string()
    } else {
        format!("{label}{}", " ".repeat(width - w))
    }
}

pub struct SettingsFormRegions {
    pub fields: [(SettingsField, Rect); SettingsField::ALL.len()],
    pub field_count: usize,
}

pub fn render_settings_form(frame: &mut Frame<'_>, area: Rect, app: &App) -> SettingsFormRegions {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            SettingsField::ALL
                .iter()
                .map(|_| Constraint::Length(ROW_HEIGHT))
                .collect::<Vec<_>>(),
        )
        .split(area);

    let mut fields = [(SettingsField::EnginePath, Rect::default()); SettingsField::ALL.len()];
    let mut field_count = 0usize;
    let label_w = label_column_width();

    for (field, row_area) in SettingsField::ALL.iter().zip(rows.iter()) {
        let focused = app.settings_field == *field;
        let value = field_value(app, *field);
        let label = pad_label(field.label(), label_w);
        let line = Line::from(vec![
            Span::styled(format!("{label}  "), if focused { highlight() } else { text_bold() }),
            Span::styled(value, if focused { highlight() } else { text_style() }),
        ]);
        frame.render_widget(
            Paragraph::new(line).block(Block::default().borders(Borders::NONE)),
            *row_area,
        );
        if field_count < fields.len() {
            fields[field_count] = (*field, *row_area);
            field_count += 1;
        }
    }

    SettingsFormRegions {
        fields,
        field_count,
    }
}

fn field_value(app: &App, field: SettingsField) -> String {
    match field {
        SettingsField::EnginePath => display_or_placeholder(&app.engine.path),
        SettingsField::EngineProtocol => app.engine.protocol.label().to_string(),
        SettingsField::EngineThreads => app.engine.threads.to_string(),
        SettingsField::EngineHashMb => app.engine.hash_mb.to_string(),
        SettingsField::EngineSkill => app.engine.skill_level.to_string(),
        SettingsField::EngineMultiPv => app.engine.multi_pv.to_string(),
        SettingsField::BookLocalPath => display_or_placeholder(&app.book.local_path),
        SettingsField::BookLocalEnabled => yes_no(app.book.local_enabled).to_string(),
        SettingsField::BookCloudEnabled => yes_no(app.book.cloud_enabled).to_string(),
        SettingsField::BookPickMode => pick_mode_label(&app.book.pick_mode).to_string(),
        SettingsField::BookMaxHalfmoves => app.book.max_halfmoves.to_string(),
    }
}

pub fn form_block(focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(if focused {
            border_focused()
        } else {
            border_normal()
        })
        .title(Span::styled(
            "设置（↑↓ 选行，空格/←→ 改值）".to_string(),
            text_bold(),
        ))
}

pub fn settings_hint(field: SettingsField) -> String {
    format!("{} {}", field.label(), field.hint())
}

