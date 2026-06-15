use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

use super::app::{
    self, source_field_schema, App, EditTarget, CONFIG_BLOCK_APPLY_DISPLAY, CONFIG_BLOCK_LIBRARY,
    CONFIG_BLOCK_ROTATION, CONFIG_BLOCK_TUI,
};
use super::line_view;
use super::sources_view;
use super::style::{self, StatusKind};

/// Descriptive title for the edit target (block or specific source with its json label+type).
/// Used for chrome block titles so "what is being edited" is obvious at a glance.
pub(super) fn edit_target_title(app: &App) -> String {
    if let Some(sess) = &app.editing {
        match &sess.target {
            EditTarget::Block(CONFIG_BLOCK_ROTATION) => "Edit Rotation".to_string(),
            EditTarget::Block(CONFIG_BLOCK_LIBRARY) => "Edit Library".to_string(),
            EditTarget::Block(CONFIG_BLOCK_APPLY_DISPLAY) => "Edit Apply/display".to_string(),
            EditTarget::Block(CONFIG_BLOCK_TUI) => "Edit TUI".to_string(),
            EditTarget::Wallhaven => "Edit Wallhaven".to_string(),
            EditTarget::SearchFilters => "Edit Search Filters".to_string(),
            EditTarget::Block(b) => format!("Edit block {}", b),
            EditTarget::Source(i) => {
                if let Some(ref src) = sess.draft_source {
                    if src.source_type == "reddit" {
                        format!("Edit Reddit #{}", i + 1)
                    } else {
                        let lab = sources_view::source_display_name(src);
                        format!("Edit Source #{}: {} ({})", i + 1, lab, src.source_type)
                    }
                } else {
                    format!("Edit source #{}", i + 1)
                }
            }
        }
    } else {
        "Config (editing)".to_string()
    }
}

/// Pure form lines for drill-down edit view.
fn config_edit_form_lines(app: &App) -> Vec<String> {
    if let Some(sess) = &app.editing {
        let mut lines: Vec<String> =
            vec!["┄─ EDIT FORM (▸ focus | ↑/↓ | type or Space/←/→ | Enter save | Esc) ─┄".into()];
        if !sess.validation_errors.is_empty() {
            lines.push("!! Validation errors:".into());
            for e in &sess.validation_errors {
                if let Some((message, hint)) = e.split_once(" (hint: ") {
                    lines.push(format!("!! - {}", message));
                    lines.push(format!("!!   hint: {}", hint.trim_end_matches(')')));
                } else {
                    lines.push(format!("!! - {}", e));
                }
            }
            lines.push("".into());
        }
        if let Some(notice) = edit_notice_line(app) {
            lines.push(notice);
            lines.push("".into());
        }

        let mut fields: Vec<(String, String, app::EditFieldKind)> = vec![];
        let mut field_keys: Vec<String> = vec![];
        if let Some(ref src) = sess.draft_source {
            for spec in source_field_schema::source_field_specs(src) {
                let value = source_field_schema::source_field_value(src, &spec.key);
                fields.push((spec.label, value, spec.kind));
                field_keys.push(spec.key);
            }
            if let Some(key) = walls_core::config::source_secrets_key(&src.source_type) {
                fields.push((
                    walls_core::config::secrets_credential_label(key).into(),
                    walls_core::config::SECRETS_EDIT_HINT.into(),
                    app::EditFieldKind::Text,
                ));
                field_keys.push(String::new());
            } else if src.source_type == "wallhaven" {
                fields.push((
                    "Wallhaven API key".into(),
                    walls_core::config::SECRETS_EDIT_HINT.into(),
                    app::EditFieldKind::Text,
                ));
                field_keys.push(String::new());
            }
        } else if matches!(
            &sess.target,
            EditTarget::Wallhaven | EditTarget::SearchFilters
        ) {
            let keys = if matches!(&sess.target, EditTarget::SearchFilters) {
                app::SEARCH_FILTER_FIELDS
            } else {
                app::WALLHAVEN_BLOCK_FIELDS
            };
            for k in keys {
                if let Some(v) = sess.draft_block_values.get(*k) {
                    let label = if *k == "purity_nsfw" && !app.wallhaven_block_field_locked(k) {
                        "Purity: NSFW".to_string()
                    } else {
                        app::block_field_label(app::WALLHAVEN_FIELDS_BLOCK, k)
                    };
                    fields.push((
                        label,
                        v.clone(),
                        app::block_field_kind(app::WALLHAVEN_FIELDS_BLOCK, k),
                    ));
                    field_keys.push((*k).into());
                }
            }
            if matches!(&sess.target, EditTarget::Wallhaven) {
                fields.push((
                    "API key".into(),
                    walls_core::config::SECRETS_EDIT_HINT.into(),
                    app::EditFieldKind::Text,
                ));
                field_keys.push(String::new());
            }
        } else if let EditTarget::Block(block) = &sess.target {
            for spec in source_field_schema::block_field_specs(*block) {
                if let Some(v) = sess.draft_block_values.get(&spec.key) {
                    fields.push((spec.label, v.clone(), spec.kind));
                    field_keys.push(String::new());
                }
            }
        }

        let max_label = fields.iter().map(|(l, _, _)| l.len()).max().unwrap_or(0);
        let pad = std::cmp::min(max_label, 28);
        for (i, (k, v, kind)) in fields.iter().enumerate() {
            let padded = format!("{:>width$}", k, width = pad);
            let field_key = field_keys.get(i).map(String::as_str).unwrap_or("");
            let val = if i == sess.field_cursor {
                match kind {
                    app::EditFieldKind::Text => format!("{}|", sess.field_buffer),
                    app::EditFieldKind::TagList => app
                        .tag_editor_display_value()
                        .unwrap_or_else(|| format!("{}  (Enter tags)", sess.field_buffer)),
                    app::EditFieldKind::Bool | app::EditFieldKind::Choice(_) => format!(
                        "‹ {} ›",
                        if let Some(src) = &sess.draft_source {
                            if src.source_type == "reddit" {
                                app.reddit_field_display_value(
                                    src,
                                    field_key,
                                    &sess.field_buffer,
                                    *kind,
                                )
                            } else {
                                app::App::choice_display_for_current_field(
                                    &sess.field_buffer,
                                    *kind,
                                )
                            }
                        } else if field_key.is_empty() {
                            app::App::choice_display_for_current_field(&sess.field_buffer, *kind)
                        } else {
                            app.wallhaven_field_display_value(field_key, &sess.field_buffer, *kind)
                        }
                    ),
                }
            } else if let Some(src) = &sess.draft_source {
                if src.source_type == "reddit" {
                    app.reddit_field_display_value(src, field_key, v, *kind)
                } else {
                    app::App::choice_display_for_current_field(v, *kind)
                }
            } else if field_key.is_empty() {
                app::App::choice_display_for_current_field(v, *kind)
            } else {
                app.wallhaven_field_display_value(field_key, v, *kind)
            };
            if i == sess.field_cursor {
                lines.push(format!("▸ {}: {}", padded, val));
            } else {
                lines.push(format!("  {}: {}", padded, val));
            }
        }
        lines
    } else {
        vec![]
    }
}

fn edit_notice_line(app: &App) -> Option<String> {
    let sess = app.editing.as_ref()?;
    let EditTarget::Source(index) = sess.target else {
        return None;
    };
    let source = sess.draft_source.as_ref()?;
    if source.source_type != "wallhaven" {
        return None;
    }

    let configured = walls_core::config::source_wallhaven_search(source);
    let key = walls_core::wallhaven::source_search_key(index, source);
    let effective = app
        .ctx
        .state
        .wallhaven
        .effective_source_searches
        .get(&key)?;
    let ratio_broadened =
        !configured.ratios.trim().is_empty() && effective.ratios.trim().is_empty();
    let resolution_broadened =
        !configured.atleast.trim().is_empty() && effective.atleast.trim().is_empty();
    if ratio_broadened || resolution_broadened {
        Some(":: Search broadened: open URL omits ratio/resolution.".into())
    } else {
        None
    }
}

fn build_rich_edit_form_items(app: &App, theme: style::Theme) -> Vec<ListItem<'static>> {
    let plain_lines = config_edit_form_lines(app);
    let mut items = Vec::new();
    for line in plain_lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("┄")
            || trimmed.starts_with("───")
            || trimmed.starts_with("─ ")
            || trimmed.starts_with("===")
        {
            let l = Line::from(Span::styled(
                line,
                theme.accent().add_modifier(Modifier::BOLD),
            ));
            items.push(ListItem::new(l));
            continue;
        }
        if trimmed.starts_with("!!") {
            let err_st = theme.status(StatusKind::Error);
            let l = Line::from(vec![
                Span::styled("!! ", err_st),
                Span::styled(line[3..].to_string(), err_st),
            ]);
            items.push(ListItem::new(l));
            continue;
        }
        if trimmed.starts_with("::") {
            let warn_st = theme.status(StatusKind::Warning);
            let l = Line::from(vec![
                Span::styled(":: ", warn_st),
                Span::styled(line[3..].to_string(), warn_st),
            ]);
            items.push(ListItem::new(l));
            continue;
        }
        if (trimmed.starts_with("▸ ") || trimmed.starts_with("  ")) && line.find(": ").is_some() {
            let colon_pos = line.find(": ").expect("checked above");
            let label_part = &line[..colon_pos];
            let value_part = &line[colon_pos + 2..];
            let is_cur = trimmed.starts_with("▸ ");
            let label_st = if is_cur {
                theme.edit_focus_label()
            } else {
                theme.muted()
            };
            let val_st = if is_cur {
                theme.edit_focus_value()
            } else if value_part == "true" {
                theme.boolean_true()
            } else if value_part == "false" {
                theme.boolean_false()
            } else if value_part.starts_with("unavailable") {
                theme.unavailable()
            } else {
                theme.normal()
            };
            let l = Line::from(vec![
                Span::styled(label_part.to_string(), label_st),
                Span::styled(
                    ": ",
                    if is_cur {
                        theme.edit_focus_row()
                    } else {
                        theme.normal()
                    },
                ),
                Span::styled(value_part.to_string(), val_st),
            ]);
            items.push(ListItem::new(l));
            continue;
        }
        let st = line_view::line_style(&line, theme);
        items.push(ListItem::new(line).style(st));
    }
    items
}

pub(super) fn render_rich_edit(
    f: &mut Frame,
    area: Rect,
    app: &App,
    theme: style::Theme,
    block_title: &str,
) {
    let items = build_rich_edit_form_items(app, theme);
    let list = List::new(items)
        .block(theme.content_block(block_title))
        .style(theme.normal());
    f.render_widget(list, area);
}
