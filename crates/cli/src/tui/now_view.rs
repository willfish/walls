use super::app::App;
use super::style;

pub(crate) fn lines(app: &App) -> Vec<String> {
    let mut lines = match &app.ctx.state.current {
        Some(c) => vec![
            format!("source: {}", c.source_id),
            format!("wallhaven: {:?}", c.wallhaven_id),
            format!("original: {}", c.original_path),
            format!("composed: {}", c.composed_path),
            app.message.clone(),
        ],
        None => vec![
            style::state_text(style::StateKind::Empty, "no current wallpaper"),
            app.message.clone(),
        ],
    };
    lines.extend(last_run_lines(app));
    lines
}

fn last_run_lines(app: &App) -> Vec<String> {
    let Ok(events) = walls_core::events::read_events(&app.ctx.paths.event_journal_file) else {
        return vec!["last run: unavailable".into()];
    };
    let Some(summary) = walls_core::events::last_run_summary(&events) else {
        return vec!["last run: (none)".into()];
    };
    let mut lines = vec![format!("last run: {}", summary.message)];
    if let Some(warning) = summary.warnings.first() {
        lines.push(format!("last warning: {warning}"));
    }
    if let Some(error) = summary.errors.first() {
        lines.push(format!("last error: {error}"));
    }
    lines
}
