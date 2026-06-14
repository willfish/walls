use super::app::App;
use super::style;

pub(crate) fn lines(app: &App) -> Vec<String> {
    match &app.ctx.state.current {
        Some(current) => current_dashboard_lines(app, current),
        None => empty_dashboard_lines(app),
    }
}

fn current_dashboard_lines(app: &App, current: &walls_core::state::CurrentWall) -> Vec<String> {
    let source = source_summary(current);
    let tags = tag_summary(current.wallhaven_id.as_deref());
    let actions = next_action_summary(current.wallhaven_id.as_deref());
    let mut lines = vec![
        format!(
            "Now  {}    rotation {}   queue {}",
            current.source_id,
            rotation_state(app),
            app.ctx.state.cache_queue.len()
        ),
        String::new(),
        format!("Status       {}", last_run_status(app)),
        format!(
            "Library      queue {}   history {}   paused {}",
            app.ctx.state.cache_queue.len(),
            app.ctx.state.history.len(),
            app.ctx.state.paused
        ),
        format!("Source       {source}   {tags}"),
        String::new(),
        format!("Next action  {actions}"),
        "             f favorite   d trash   o open".into(),
        String::new(),
        format!("Paths        original {}", current.original_path),
        format!("             composed {}", current.composed_path),
    ];
    lines.extend(last_run_detail_lines(app));
    lines
}

fn empty_dashboard_lines(app: &App) -> Vec<String> {
    let mut lines = vec![
        style::state_text(style::StateKind::Empty, "no current wallpaper"),
        String::new(),
        format!("Status       {}", last_run_status(app)),
        format!(
            "Library      queue {}   history {}   paused {}",
            app.ctx.state.cache_queue.len(),
            app.ctx.state.history.len(),
            app.ctx.state.paused
        ),
        "Source       unavailable   no wallpaper selected".into(),
        String::new(),
        "Next action  n next wallpaper   / search   Browse candidates".into(),
    ];
    lines.extend(last_run_detail_lines(app));
    lines
}

fn rotation_state(app: &App) -> &'static str {
    if app.ctx.config.change.enabled {
        "on"
    } else {
        "off"
    }
}

fn source_summary(current: &walls_core::state::CurrentWall) -> String {
    match (
        current.provider.as_deref(),
        current
            .wallhaven_id
            .as_deref()
            .filter(|id| !id.trim().is_empty()),
    ) {
        (Some("wallhaven"), Some(id)) => format!("Wallhaven {id}"),
        (Some(provider), Some(id)) => format!("{provider} {id}"),
        (Some(provider), None) => provider.to_string(),
        (None, Some(id)) => format!("Wallhaven {id}"),
        (None, None) => current.source_id.clone(),
    }
}

fn tag_summary(wallhaven_id: Option<&str>) -> &'static str {
    match wallhaven_id {
        Some(id) if !id.trim().is_empty() => "tags available",
        _ => "tags unavailable",
    }
}

fn next_action_summary(wallhaven_id: Option<&str>) -> &'static str {
    match wallhaven_id {
        Some(id) if !id.trim().is_empty() => "c create source from current",
        _ => "f favorite current wallpaper",
    }
}

fn last_run_status(app: &App) -> String {
    let Ok(events) = walls_core::events::read_events(&app.ctx.paths.event_journal_file) else {
        return "last run unavailable".into();
    };
    let Some(summary) = walls_core::events::last_run_summary(&events) else {
        return "last run none".into();
    };
    summary.message
}

fn last_run_detail_lines(app: &App) -> Vec<String> {
    let Ok(events) = walls_core::events::read_events(&app.ctx.paths.event_journal_file) else {
        return vec!["             last run unavailable".into()];
    };
    let Some(summary) = walls_core::events::last_run_summary(&events) else {
        return vec!["             last run none".into()];
    };
    let mut lines = Vec::new();
    if let Some(warning) = summary.warnings.first() {
        lines.push(format!("             warning {warning}"));
    }
    if let Some(error) = summary.errors.first() {
        lines.push(format!("             error {error}"));
    }
    lines
}
