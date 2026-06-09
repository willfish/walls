use ratatui::prelude::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use super::style;

const CONFIG_DETAIL_LABEL_WIDTH: usize = 20;

pub(crate) fn section_detail_item(
    title: impl Into<String>,
    theme: style::Theme,
) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::raw("    "),
        Span::styled(format!("─ {}", title.into()), theme.accent()),
    ]))
}

pub(crate) fn key_value_detail_item(
    label: impl Into<String>,
    value: impl Into<String>,
    theme: style::Theme,
) -> ListItem<'static> {
    config_detail_item("    ", label, value, theme, theme.normal())
}

pub(crate) fn detected_detail_item(
    label: impl Into<String>,
    value: impl Into<String>,
    theme: style::Theme,
) -> ListItem<'static> {
    config_detail_item("    · ", label, value, theme, theme.normal())
}

pub(crate) fn path_detail_item(
    label: impl Into<String>,
    value: impl Into<String>,
    theme: style::Theme,
) -> ListItem<'static> {
    config_detail_item("    ", label, value, theme, theme.muted())
}

fn config_detail_item(
    prefix: &'static str,
    label: impl Into<String>,
    value: impl Into<String>,
    theme: style::Theme,
    fallback_value_style: Style,
) -> ListItem<'static> {
    let label = label.into();
    let value = value.into();
    let value_style = config_value_style(&value, theme).unwrap_or(fallback_value_style);
    ListItem::new(Line::from(vec![
        Span::raw(prefix),
        Span::styled(
            format!("{label:<CONFIG_DETAIL_LABEL_WIDTH$}: "),
            theme.muted(),
        ),
        Span::styled(value, value_style),
    ]))
}

pub(crate) fn warning_detail_item(
    warning: impl Into<String>,
    theme: style::Theme,
) -> ListItem<'static> {
    let warning = warning.into();
    let text = warning.strip_prefix("warning: ").unwrap_or(&warning);
    ListItem::new(Line::from(vec![
        Span::raw("    "),
        Span::styled("! ", theme.unavailable()),
        Span::styled(text.to_string(), theme.unavailable()),
    ]))
}

pub(crate) fn spacer_detail_item() -> ListItem<'static> {
    ListItem::new("")
}

fn config_value_style(value: &str, theme: style::Theme) -> Option<Style> {
    match value {
        "true" | "on" => Some(theme.boolean_true()),
        "false" | "off" | "disabled" => Some(theme.boolean_false()),
        value if value.starts_with("unavailable") => Some(theme.unavailable()),
        _ => None,
    }
}
