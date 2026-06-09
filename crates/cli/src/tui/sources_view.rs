//! Sources block layout for the Config tab.

use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;
use walls_core::config::{reddit_summary, source_secrets_detail_lines, SourceEntry};

use super::app::App;
use super::style::{self, StateKind, Theme};

const SOURCE_LABEL_WIDTH: usize = 26;

pub fn sources_block_summary(app: &App) -> String {
    let sources = &app.ctx.config.sources;
    let total = sources.len();
    let active = sources.iter().filter(|s| s.enabled).count();
    format!("{active} active · {total} total")
}

pub fn sources_detail_lines(app: &App) -> Vec<String> {
    let mut lines = Vec::new();
    for item in source_rows(app) {
        lines.push(item.plain);
        lines.extend(item.detail_lines.into_iter().map(|d| format!("      {d}")));
    }
    if let Some(hint) = disabled_hint(app) {
        lines.push(hint);
    }
    lines
}

pub fn build_sources_list_items(app: &App, theme: Theme, indent: usize) -> Vec<ListItem<'static>> {
    let pad = " ".repeat(indent);
    let mut items = Vec::new();
    for row in source_rows(app) {
        items.push(ListItem::new(prefixed_line(&pad, row_spans(&row, theme))));
        for detail in row.detail_lines {
            items.push(source_detail_item(&pad, &detail, theme));
        }
    }
    if let Some(hint) = disabled_hint(app) {
        items.push(ListItem::new(Line::from(vec![
            Span::raw(pad),
            Span::styled(
                style::state_text(StateKind::Disabled, hint.trim()),
                theme.state(StateKind::Disabled),
            ),
        ])));
    }
    items
}

fn source_detail_item(pad: &str, detail: &str, theme: Theme) -> ListItem<'static> {
    if let Some(text) = detail.strip_prefix("warning: ") {
        return ListItem::new(prefixed_line(
            pad,
            style::state_line(StateKind::ValidationWarning, text.to_string(), theme),
        ));
    }
    if let Some((label, value)) = detail.split_once(": ") {
        if value == "missing" {
            return ListItem::new(Line::from(vec![
                Span::raw(pad.to_string()),
                Span::raw("      "),
                Span::styled(format!("{label}: "), theme.muted()),
                Span::styled("[missing]", theme.state(StateKind::MissingConfig)),
            ]));
        }
        if let Some((kind, message)) = style::state_parts(value) {
            return ListItem::new(Line::from(vec![
                Span::raw(pad.to_string()),
                Span::raw("      "),
                Span::styled(format!("{label}: "), theme.muted()),
                Span::styled(format!("[{}] ", kind.label()), theme.state(kind)),
                Span::styled(message.to_string(), theme.state(kind)),
            ]));
        }
    }
    ListItem::new(Line::from(vec![
        Span::raw(pad.to_string()),
        Span::styled(format!("      {detail}"), theme.muted()),
    ]))
}

fn prefixed_line(pad: &str, line: Line<'static>) -> Line<'static> {
    let mut spans = vec![Span::raw(pad.to_string())];
    spans.extend(line.spans);
    Line::from(spans)
}

struct SourceRow {
    selected: bool,
    enabled: bool,
    label: String,
    meta: String,
    plain: String,
    detail_lines: Vec<String>,
}

fn source_rows(app: &App) -> Vec<SourceRow> {
    let sources = &app.ctx.config.sources;
    let browse_all = show_all_sources(app);
    let sub_sel = subnav_selection(app);
    let mut rows = Vec::new();

    for (index, src) in sources.iter().enumerate() {
        if !browse_all && !src.enabled {
            continue;
        }
        let label = source_display_name(src);
        let meta = source_display_meta(src);
        let selected = sub_sel == Some(index);
        let detail_lines = if selected {
            source_secrets_detail_lines(
                src,
                &app.ctx.secrets,
                app.ctx.config.change.internet_enabled,
            )
        } else {
            Vec::new()
        };
        rows.push(SourceRow {
            selected,
            enabled: src.enabled,
            label: label.clone(),
            meta: meta.clone(),
            plain: format_source_row(selected, src.enabled, &label, &meta),
            detail_lines,
        });
    }

    rows
}

fn show_all_sources(app: &App) -> bool {
    app.config_in_subnav && app.is_sources_list_block(app.config_cursor)
}

fn subnav_selection(app: &App) -> Option<usize> {
    if show_all_sources(app) {
        Some(app.config_sub_cursor)
    } else {
        None
    }
}

fn disabled_hint(app: &App) -> Option<String> {
    if show_all_sources(app) {
        return None;
    }
    let disabled = app.ctx.config.sources.iter().filter(|s| !s.enabled).count();
    if disabled == 0 {
        return None;
    }
    let noun = if disabled == 1 { "source" } else { "sources" };
    Some(format!("{disabled} disabled {noun}; Enter to browse all"))
}

fn format_source_row(selected: bool, enabled: bool, label: &str, meta: &str) -> String {
    let marker = if selected { "▸ " } else { "  " };
    let state = if enabled { "on" } else { "off" };
    let padded = if label.chars().count() >= SOURCE_LABEL_WIDTH {
        label.to_string()
    } else {
        pad_label(label, SOURCE_LABEL_WIDTH)
    };
    format!("  {marker}{padded} {state} · {meta}")
}

fn pad_label(label: &str, width: usize) -> String {
    let visible = label.chars().count();
    if visible >= width {
        return label.to_string();
    }
    format!("{label}{: <width$}", "", width = width - visible)
}

fn row_spans(row: &SourceRow, theme: Theme) -> Line<'static> {
    let marker = if row.selected { "▸ " } else { "  " };
    let padded = if row.label.chars().count() >= SOURCE_LABEL_WIDTH {
        row.label.clone()
    } else {
        pad_label(&row.label, SOURCE_LABEL_WIDTH)
    };
    let marker_style = if row.selected {
        theme.selected()
    } else {
        theme.normal()
    };
    let label_style = if row.selected {
        theme.selected()
    } else if row.enabled {
        theme.heading()
    } else {
        theme.muted()
    };
    let state_style = if row.enabled {
        theme.active_state()
    } else {
        theme.inactive_state()
    };
    Line::from(
        vec![
            Span::raw("  "),
            Span::styled(marker.to_string(), marker_style),
            Span::styled(padded, label_style),
            Span::raw(" "),
            Span::styled(if row.enabled { "on" } else { "off" }, state_style),
            Span::styled(" · ", theme.muted()),
        ]
        .into_iter()
        .chain(source_meta_spans(&row.meta, theme))
        .collect::<Vec<_>>(),
    )
}

fn source_meta_spans(meta: &str, theme: Theme) -> Vec<Span<'static>> {
    if let Some((kind, rest)) = style::state_parts(meta) {
        return vec![
            Span::styled(format!("[{}] ", kind.label()), theme.state(kind)),
            Span::styled(rest.to_string(), theme.state(kind)),
        ];
    }
    vec![Span::styled(meta.to_string(), theme.muted())]
}

pub fn source_display_name(src: &SourceEntry) -> String {
    if src.source_type == "wallhaven" {
        return src
            .query
            .as_deref()
            .filter(|query| !query.trim().is_empty())
            .map(|query| format!("Wallhaven {query}"))
            .unwrap_or_else(|| "Wallhaven".into());
    }
    if let Some(label) = src.label.as_deref().filter(|l| !l.is_empty()) {
        if label.to_ascii_lowercase() != src.source_type {
            return label.to_string();
        }
    }
    match src.source_type.as_str() {
        "reddit" => "Reddit".into(),
        "favorites" => "Favorites".into(),
        "fetched" => "Fetched".into(),
        "folder" => "Local folder".into(),
        "image" => "Local image".into(),
        "unsplash" => "Unsplash".into(),
        "bing" => "Bing daily".into(),
        "apod" => "NASA APOD".into(),
        "mediarss" => "Media RSS".into(),
        "json" => "JSON feed".into(),
        "pixabay" => "Pixabay".into(),
        "immich" => "Immich".into(),
        "spotlight" => "Spotlight".into(),
        "weighting" => "Weighting".into(),
        "wallhaven" => "Wallhaven".into(),
        "attribution" => "Attribution".into(),
        other => title_case_type(other),
    }
}

fn title_case_type(value: &str) -> String {
    if value.is_empty() {
        return "Source".into();
    }
    let mut chars = value.chars();
    let first = chars.next().unwrap().to_ascii_uppercase().to_string();
    format!("{first}{}", chars.as_str())
}

pub fn source_display_meta(src: &SourceEntry) -> String {
    match src.source_type.as_str() {
        "reddit" => reddit_summary(src),
        "favorites" | "fetched" => "local library".into(),
        "folder" | "image" => src
            .path
            .as_deref()
            .map(|path| truncate_middle(path, 36))
            .unwrap_or_else(|| style::state_text(StateKind::MissingConfig, "path not set")),
        "unsplash" => {
            let query = src.query.as_deref().unwrap_or("any query");
            let orientation = src.orientation.as_deref().unwrap_or("any");
            format!("{query} · {orientation}")
        }
        "bing" => "image of the day".into(),
        "apod" => "NASA feed".into(),
        "json" => src
            .url
            .as_deref()
            .map(|url| truncate_middle(url, 36))
            .unwrap_or_else(|| style::state_text(StateKind::MissingConfig, "URL not set")),
        "mediarss" => src
            .url
            .as_deref()
            .map(|url| truncate_middle(url, 36))
            .unwrap_or_else(|| style::state_text(StateKind::MissingConfig, "feed URL not set")),
        "pixabay" => src
            .query
            .as_deref()
            .map(|q| format!("{q} images"))
            .unwrap_or_else(|| "images".into()),
        "immich" => src
            .url
            .as_deref()
            .map(|url| truncate_middle(url, 36))
            .unwrap_or_else(|| style::state_text(StateKind::MissingConfig, "server not set")),
        "spotlight" => "Windows Spotlight".into(),
        "weighting" => src
            .query
            .as_deref()
            .unwrap_or("priority weight")
            .to_string(),
        "wallhaven" => src
            .query
            .as_deref()
            .filter(|query| !query.trim().is_empty())
            .map(|query| format!("query {}", short_query(query)))
            .unwrap_or_else(|| style::state_text(StateKind::MissingConfig, "query not set")),
        "attribution" => "custom URL".into(),
        _ => src
            .url
            .as_deref()
            .or(src.query.as_deref())
            .or(src.path.as_deref())
            .map(|v| truncate_middle(v, 36))
            .unwrap_or_else(|| style::state_text(StateKind::MissingConfig, "not configured")),
    }
}

fn short_query(query: &str) -> String {
    truncate_middle(query, 24)
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return "...".into();
    }
    let keep = max_chars - 3;
    let front = keep / 2;
    let back = keep - front;
    format!(
        "{}...{}",
        chars.iter().take(front).collect::<String>(),
        chars.iter().skip(chars.len() - back).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_display_name_avoids_redundant_type_suffix() {
        let src = SourceEntry {
            enabled: true,
            source_type: "folder".into(),
            label: Some("My wallpapers".into()),
            path: Some("/tmp".into()),
            query: None,
            url: None,
            collection: None,
            user: None,
            topic: None,
            orientation: None,
            api_key: None,
            image_path: None,
            title_path: None,
            sort: None,
            time: None,
            ..SourceEntry::default()
        };
        assert_eq!(source_display_name(&src), "My wallpapers");
    }

    #[test]
    fn wallhaven_source_display_name_derives_from_query() {
        let src = SourceEntry {
            enabled: true,
            source_type: "wallhaven".into(),
            label: None,
            path: None,
            query: Some("jupiter".into()),
            url: None,
            collection: None,
            user: None,
            topic: None,
            orientation: None,
            api_key: None,
            image_path: None,
            title_path: None,
            sort: None,
            time: None,
            ..SourceEntry::default()
        };

        assert_eq!(source_display_name(&src), "Wallhaven jupiter");
        assert_eq!(source_display_meta(&src), "query jupiter");
    }
}
