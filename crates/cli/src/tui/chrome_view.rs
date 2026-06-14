use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::app::{App, InputMode, Tab};
use super::style;

pub(crate) fn key_help_lines(app: &App, width: u16) -> Vec<String> {
    let compact = width < 70;
    let mut lines = vec![
        "Global".into(),
        "  Esc/q close help".into(),
        "  ? help   q quit   o open selected   n/p next/prev   Space pause".into(),
        "  f favorite current   d request trash current   Shift+X reset provider storage".into(),
        "Tabs and lists".into(),
        "  Emacs: 1-6 or ←/→ tabs   j/k or ↑/↓ move".into(),
        "  Vim: 1-6 or h/l tabs   j/k move   gg/G first/last".into(),
        "  Home/End first/last   PageUp/PageDown jump   Enter apply/open".into(),
        "Search".into(),
        "  / opens Search input from normal mode; i edits from Search tab".into(),
        "  Search filters: e edits query, categories, purity, sorting, order, minimum resolution locally".into(),
        "  Search input: type, Backspace, Enter search, Esc cancel".into(),
        "Command mode".into(),
        "  : opens commands; Ctrl+n/Ctrl+p completes; Enter runs; Esc cancels".into(),
        "  :next :prev :pause :favorite :source from-current :status :quit".into(),
        "Config".into(),
        "  Sources: a adds a Wallhaven query; x removes selected removable source; e edits first active; Enter picks a source; Esc leaves subnav".into(),
        "  Config values: e edit   t toggle".into(),
        "Config edit".into(),
        "  ↑/↓ fields   text keys type   Backspace deletes".into(),
        "  Space or ←/→ cycle bool/choice fields   Enter save   Esc cancel".into(),
        "Destructive confirmations".into(),
        "  Trash prompt: d confirm   Esc cancel".into(),
        "  Provider reset prompt: Shift+X confirm   Esc cancel".into(),
        format!("Current mode: {}", key_help_mode_label(app)),
    ];

    if compact {
        lines.retain(|line| {
            !line.contains("Home/End")
                && !line.contains(":next")
                && !line.contains("Backspace deletes")
                && !line.contains("Trash prompt")
                && !line.contains("Provider reset prompt")
        });
    }

    lines
}

pub(crate) fn footer_paragraph(app: &App, width: u16, theme: style::Theme) -> Paragraph<'_> {
    let mode = match app.input_mode {
        InputMode::Normal => "normal",
        InputMode::Command => "command",
        InputMode::SearchInput => "search",
    };
    let status = if app.message.is_empty() {
        "ready".to_string()
    } else {
        app.message.clone()
    };
    let status_line = format!(
        "{status} | paused={} | queue={} | history={}",
        app.ctx.state.paused,
        app.ctx.state.cache_queue.len(),
        app.ctx.state.history.len()
    );
    let status_kind = if app.message.is_empty() {
        style::StatusKind::Neutral
    } else {
        app.message_kind
    };

    Paragraph::new(vec![
        Line::from(vec![
            Span::styled(format!("{mode} "), theme.accent()),
            Span::styled(status_line, theme.status(status_kind)),
        ]),
        Line::from(vec![Span::styled(
            footer_keys(app, width),
            theme.key_hint(),
        )]),
    ])
    .block(theme.chrome_block("keys"))
}

pub(crate) fn footer_keys(app: &App, width: u16) -> String {
    if width < 50 {
        return app.compact_footer_keys();
    }

    app.footer_keys()
}

fn key_help_mode_label(app: &App) -> &'static str {
    if app.pending_trash_confirm {
        "trash confirmation"
    } else if app.pending_nuke_confirm {
        "provider reset confirmation"
    } else if app.is_editing() {
        "config edit"
    } else {
        match app.input_mode {
            InputMode::Command => "command input",
            InputMode::SearchInput => "search input",
            InputMode::Normal if app.tab == Tab::Config && app.config_in_subnav => {
                "config sources subnav"
            }
            InputMode::Normal => "normal",
        }
    }
}
