use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::widgets::{List, ListItem};

use super::style;

pub(crate) fn render_lines(
    f: &mut Frame,
    area: Rect,
    title: &str,
    body: Vec<String>,
    theme: style::Theme,
) {
    let items: Vec<ListItem> = body
        .iter()
        .map(|line| string_list_item(line, theme))
        .collect();
    let list = List::new(items)
        .block(theme.content_block(title))
        .style(theme.normal());
    f.render_widget(list, area);
}

fn string_list_item(line: &str, theme: style::Theme) -> ListItem<'static> {
    if let Some((kind, message)) = style::state_parts(line) {
        return ListItem::new(style::state_line(kind, message.to_string(), theme));
    }
    ListItem::new(line.to_string()).style(line_style(line, theme))
}

pub(crate) fn line_style(line: &str, theme: style::Theme) -> Style {
    let trimmed = line.trim_start();
    if let Some((kind, _)) = style::state_parts(trimmed) {
        return theme.state(kind);
    }
    if trimmed.starts_with('>') || trimmed.starts_with("▸ ") {
        return theme.selected();
    }
    if trimmed.starts_with("Edit ") {
        // Edit form titles pop with accent (cyan bold in colour mode) for hierarchy.
        return theme.accent();
    }
    if trimmed.starts_with("┄")
        || trimmed.starts_with("───")
        || trimmed.starts_with("─ ")
        || trimmed.starts_with("===")
    {
        // Modern separator/header with box chars: bold muted (calm, legible in no-colour).
        return theme.muted().add_modifier(Modifier::BOLD);
    }
    if trimmed.starts_with("--") {
        return theme.muted();
    }
    if trimmed.starts_with('(') || trimmed.contains("preview unavailable") {
        return theme.muted();
    }
    if trimmed.starts_with("!!") {
        // Validation errors ... red/bold inline.
        return theme.status(style::StatusKind::Error);
    }
    theme.normal()
}
