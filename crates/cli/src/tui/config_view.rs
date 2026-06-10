use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};
use walls_core::apply::{
    backend_setting_label, summarize_apply_environment, ApplyEnvironmentSummary,
};
use walls_core::config::{ApplyBackendSetting, CosmicMethod, TuiKeyProfile};

use super::app::{
    App, CONFIG_BLOCK_APPLY_DISPLAY, CONFIG_BLOCK_LIBRARY, CONFIG_BLOCK_ROTATION,
    CONFIG_BLOCK_SOURCES, CONFIG_BLOCK_TUI,
};
use super::config_detail_view::{
    detected_detail_item, key_value_detail_item, path_detail_item, section_detail_item,
    spacer_detail_item, warning_detail_item,
};
use super::sources_view;
use super::style;

struct ConfigBlock<'a> {
    index: usize,
    cursor: usize,
    title: &'a str,
    enabled: bool,
    summary: String,
    details: Vec<ListItem<'static>>,
    theme: style::Theme,
}

pub(super) fn render_tab(f: &mut Frame, area: Rect, app: &App, theme: style::Theme) {
    let items = list_items(app, theme);
    let list = List::new(items)
        .block(theme.content_block("Config"))
        .style(theme.normal());
    f.render_widget(list, area);
}

pub(super) fn lines(app: &App) -> Vec<String> {
    let mut lines = Vec::new();
    let sources = &app.ctx.config.sources;
    let sources_enabled = sources.iter().any(|s| s.enabled);
    push_config_block(
        &mut lines,
        CONFIG_BLOCK_SOURCES,
        app.config_cursor,
        "Sources",
        sources_enabled,
        sources_view::sources_block_summary(app),
        sources_view::sources_detail_lines(app),
    );
    push_config_block(
        &mut lines,
        CONFIG_BLOCK_ROTATION,
        app.config_cursor,
        "Rotation",
        app.ctx.config.change.enabled,
        format!(
            "every {}s, {}, {:.0}% online",
            app.ctx.config.change.interval_secs,
            if app.ctx.config.change.internet_enabled {
                "online"
            } else {
                "local only"
            },
            app.ctx.config.change.download_preference_ratio * 100.0
        ),
        rotation_details(app),
    );
    push_config_block(
        &mut lines,
        CONFIG_BLOCK_LIBRARY,
        app.config_cursor,
        "Library",
        app.ctx.config.quota.enabled,
        format!(
            "{} queued, {} history, quota {}",
            app.ctx.state.cache_queue.len(),
            app.ctx.state.history.len(),
            quota_summary(app)
        ),
        library_details(app),
    );
    push_config_block(
        &mut lines,
        CONFIG_BLOCK_APPLY_DISPLAY,
        app.config_cursor,
        "Apply/display",
        true,
        format!(
            "{} backend, {} mode, {}",
            apply_block_backend_summary(app),
            app.ctx.config.display.mode,
            display_target_summary(app)
        ),
        apply_display_details(app),
    );
    push_config_block(
        &mut lines,
        CONFIG_BLOCK_TUI,
        app.config_cursor,
        "TUI",
        true,
        format!(
            "{} keys",
            tui_key_profile_label(app.ctx.config.tui.key_profile)
        ),
        tui_details(app),
    );
    lines
}

fn list_items(app: &App, theme: style::Theme) -> Vec<ListItem<'static>> {
    let mut items = Vec::new();
    let sources = &app.ctx.config.sources;
    let sources_enabled = sources.iter().any(|s| s.enabled);
    let sources_details = if app.config_cursor == CONFIG_BLOCK_SOURCES {
        sources_view::build_sources_list_items(app, theme, 4)
    } else {
        Vec::new()
    };
    push_config_block_items(
        &mut items,
        ConfigBlock {
            index: CONFIG_BLOCK_SOURCES,
            cursor: app.config_cursor,
            title: "Sources",
            enabled: sources_enabled,
            summary: sources_view::sources_block_summary(app),
            details: sources_details,
            theme,
        },
    );
    push_config_block_items(
        &mut items,
        ConfigBlock {
            index: CONFIG_BLOCK_ROTATION,
            cursor: app.config_cursor,
            title: "Rotation",
            enabled: app.ctx.config.change.enabled,
            summary: format!(
                "every {}s, {}, {:.0}% online",
                app.ctx.config.change.interval_secs,
                if app.ctx.config.change.internet_enabled {
                    "online"
                } else {
                    "local only"
                },
                app.ctx.config.change.download_preference_ratio * 100.0
            ),
            details: rotation_detail_items(app, theme),
            theme,
        },
    );
    push_config_block_items(
        &mut items,
        ConfigBlock {
            index: CONFIG_BLOCK_LIBRARY,
            cursor: app.config_cursor,
            title: "Library",
            enabled: app.ctx.config.quota.enabled,
            summary: format!(
                "{} queued, {} history, quota {}",
                app.ctx.state.cache_queue.len(),
                app.ctx.state.history.len(),
                quota_summary(app)
            ),
            details: library_detail_items(app, theme),
            theme,
        },
    );
    push_config_block_items(
        &mut items,
        ConfigBlock {
            index: CONFIG_BLOCK_APPLY_DISPLAY,
            cursor: app.config_cursor,
            title: "Apply/display",
            enabled: true,
            summary: format!(
                "{} backend, {} mode, {}",
                apply_block_backend_summary(app),
                app.ctx.config.display.mode,
                display_target_summary(app)
            ),
            details: apply_display_detail_items(app, theme),
            theme,
        },
    );
    push_config_block_items(
        &mut items,
        ConfigBlock {
            index: CONFIG_BLOCK_TUI,
            cursor: app.config_cursor,
            title: "TUI",
            enabled: true,
            summary: format!(
                "{} keys",
                tui_key_profile_label(app.ctx.config.tui.key_profile)
            ),
            details: tui_detail_items(app, theme),
            theme,
        },
    );
    items
}

fn push_config_block_items(items: &mut Vec<ListItem<'static>>, block: ConfigBlock<'_>) {
    let marker = if block.cursor == block.index {
        ">"
    } else {
        " "
    };
    let state = if block.enabled { "on" } else { "off" };
    let selected = block.cursor == block.index;
    let marker_style = if selected {
        block.theme.selected()
    } else {
        block.theme.normal()
    };
    let title_style = if selected {
        block.theme.selected()
    } else if block.enabled {
        block.theme.heading()
    } else {
        block.theme.muted()
    };
    let state_style = if block.enabled {
        block.theme.active_state()
    } else {
        block.theme.inactive_state()
    };
    items.push(ListItem::new(Line::from(vec![
        Span::styled(marker.to_string(), marker_style),
        Span::raw(" ["),
        Span::styled(state.to_string(), state_style),
        Span::raw("] "),
        Span::styled(block.title.to_string(), title_style),
        Span::styled(" - ", block.theme.muted()),
        Span::styled(block.summary, block.theme.muted()),
    ])));
    if block.cursor == block.index {
        items.extend(block.details);
    }
}

fn push_config_block(
    lines: &mut Vec<String>,
    index: usize,
    cursor: usize,
    title: &str,
    enabled: bool,
    summary: String,
    details: impl IntoIterator<Item = String>,
) {
    let marker = if cursor == index { ">" } else { " " };
    let state = if enabled { "on" } else { "off" };
    lines.push(format!("{marker} [{state}] {title} - {summary}"));
    if cursor == index {
        for detail in details {
            lines.push(format!("    {detail}"));
        }
    }
}

#[allow(dead_code)]
fn local_source_details(app: &App) -> Vec<String> {
    if app.local_source_summaries.is_empty() {
        return vec!["no local sources configured".into()];
    }

    app.local_source_summaries
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let state = if source.enabled { "on" } else { "off" };
            let plural = if source.candidates == 1 {
                "candidate"
            } else {
                "candidates"
            };
            format!(
                "{}. [{state}] {} ({}) - {} - {} {plural} - {}",
                index + 1,
                source.label,
                source.source_type,
                source.status,
                source.candidates,
                source.path,
            )
        })
        .collect()
}

fn rotation_details(app: &App) -> Vec<String> {
    vec![
        format!("enabled: {}", app.ctx.config.change.enabled),
        format!("on start: {}", app.ctx.config.change.on_start),
        format!("interval: {}s", app.ctx.config.change.interval_secs),
        format!("internet: {}", app.ctx.config.change.internet_enabled),
        format!("safe mode: {}", app.ctx.config.change.safe_mode),
        format!("lock screen: {}", app.ctx.config.change.change_lock_screen),
        format!(
            "download preference: {:.0}% online",
            app.ctx.config.change.download_preference_ratio * 100.0
        ),
        format!(
            "tray icon: {}",
            walls_core::tray_icon::tray_accent_label(walls_core::tray_icon::effective_tray_accent(
                app.ctx.config.tray.accent,
            ))
        ),
        {
            let desktop = walls_core::autostart::current_autostart_desktop();
            if walls_core::autostart::tray_autostart_available(desktop) {
                format!(
                    "tray autostart: {}",
                    walls_core::autostart::tray_autostart_enabled_for_desktop(
                        &app.ctx.config,
                        desktop
                    )
                )
            } else {
                format!(
                    "tray autostart: unavailable on {}",
                    walls_core::tray::desktop_display_name(desktop)
                )
            }
        },
    ]
}

fn rotation_detail_items(app: &App, theme: style::Theme) -> Vec<ListItem<'static>> {
    let mut items = vec![
        section_detail_item("configured", theme),
        key_value_detail_item("enabled", app.ctx.config.change.enabled.to_string(), theme),
        key_value_detail_item(
            "on start",
            app.ctx.config.change.on_start.to_string(),
            theme,
        ),
        key_value_detail_item(
            "interval",
            format!("{}s", app.ctx.config.change.interval_secs),
            theme,
        ),
        key_value_detail_item(
            "internet",
            app.ctx.config.change.internet_enabled.to_string(),
            theme,
        ),
        key_value_detail_item(
            "safe mode",
            app.ctx.config.change.safe_mode.to_string(),
            theme,
        ),
        key_value_detail_item(
            "lock screen",
            app.ctx.config.change.change_lock_screen.to_string(),
            theme,
        ),
        key_value_detail_item(
            "download preference",
            format!(
                "{:.0}% online",
                app.ctx.config.change.download_preference_ratio * 100.0
            ),
            theme,
        ),
        key_value_detail_item(
            "tray icon",
            walls_core::tray_icon::tray_accent_label(walls_core::tray_icon::effective_tray_accent(
                app.ctx.config.tray.accent,
            )),
            theme,
        ),
    ];
    let desktop = walls_core::autostart::current_autostart_desktop();
    let tray_autostart = if walls_core::autostart::tray_autostart_available(desktop) {
        walls_core::autostart::tray_autostart_enabled_for_desktop(&app.ctx.config, desktop)
            .to_string()
    } else {
        format!(
            "unavailable on {}",
            walls_core::tray::desktop_display_name(desktop)
        )
    };
    items.push(key_value_detail_item(
        "tray autostart",
        tray_autostart,
        theme,
    ));
    items
}

fn library_details(app: &App) -> Vec<String> {
    let mut details = vec![
        format!("cache: {}", app.ctx.paths.cache_dir.display()),
        format!("downloaded: {}", app.ctx.paths.download_dir.display()),
        format!("favorites: {}", app.ctx.paths.favorites_dir.display()),
        format!("fetched: {}", app.ctx.paths.fetched_dir.display()),
        format!("compose: {}", app.ctx.paths.compose_dir.display()),
        format!("quota: {}", quota_summary(app)),
        format!("queue: {} items", app.ctx.state.cache_queue.len()),
        format!("history: {} entries", app.ctx.state.history.len()),
        format!("selection: {:?}", app.ctx.config.selection.strategy),
        format!(
            "landscape filter: {}",
            app.ctx.config.selection.use_landscape_enabled
        ),
        format!("avoid recent: {}", app.ctx.config.selection.avoid_recent),
        format!(
            "refetch below: {} cached",
            app.ctx.config.selection.refetch_when_cache_below
        ),
    ];
    details.extend(config_warning_lines(app, &["quota."]));
    details
}

fn library_detail_items(app: &App, theme: style::Theme) -> Vec<ListItem<'static>> {
    let mut items = vec![
        section_detail_item("paths", theme),
        path_detail_item(
            "cache",
            app.ctx.paths.cache_dir.display().to_string(),
            theme,
        ),
        path_detail_item(
            "downloaded",
            app.ctx.paths.download_dir.display().to_string(),
            theme,
        ),
        path_detail_item(
            "favorites",
            app.ctx.paths.favorites_dir.display().to_string(),
            theme,
        ),
        path_detail_item(
            "fetched",
            app.ctx.paths.fetched_dir.display().to_string(),
            theme,
        ),
        path_detail_item(
            "compose",
            app.ctx.paths.compose_dir.display().to_string(),
            theme,
        ),
        spacer_detail_item(),
        section_detail_item("cache state", theme),
        key_value_detail_item("quota", quota_summary(app), theme),
        key_value_detail_item(
            "queue",
            format!("{} items", app.ctx.state.cache_queue.len()),
            theme,
        ),
        key_value_detail_item(
            "history",
            format!("{} entries", app.ctx.state.history.len()),
            theme,
        ),
        spacer_detail_item(),
        section_detail_item("selection", theme),
        key_value_detail_item(
            "strategy",
            format!("{:?}", app.ctx.config.selection.strategy),
            theme,
        ),
        key_value_detail_item(
            "landscape filter",
            app.ctx.config.selection.use_landscape_enabled.to_string(),
            theme,
        ),
        key_value_detail_item(
            "avoid recent",
            app.ctx.config.selection.avoid_recent.to_string(),
            theme,
        ),
        key_value_detail_item(
            "refetch below",
            format!(
                "{} cached",
                app.ctx.config.selection.refetch_when_cache_below
            ),
            theme,
        ),
    ];
    items.extend(
        config_warning_lines(app, &["quota."])
            .into_iter()
            .map(|warning| warning_detail_item(warning, theme)),
    );
    items
}

fn tui_key_profile_label(profile: TuiKeyProfile) -> &'static str {
    match profile {
        TuiKeyProfile::Emacs => "emacs",
        TuiKeyProfile::Vim => "vim",
    }
}

fn tui_details(app: &App) -> Vec<String> {
    match app.ctx.config.tui.key_profile {
        TuiKeyProfile::Emacs => vec![
            "key profile: emacs".into(),
            "tabs: ←/→ or 1-6".into(),
            "rows: j/k, arrows, Pg, Home/End".into(),
            "commands: : then Ctrl+n/Ctrl+p completes".into(),
        ],
        TuiKeyProfile::Vim => vec![
            "key profile: vim".into(),
            "tabs: h/l or 1-6".into(),
            "rows: j/k, Pg, gg/G".into(),
            "commands: : then Ctrl+n/Ctrl+p completes".into(),
        ],
    }
}

fn tui_detail_items(app: &App, theme: style::Theme) -> Vec<ListItem<'static>> {
    let mut items = vec![
        section_detail_item("configured", theme),
        key_value_detail_item(
            "key profile",
            tui_key_profile_label(app.ctx.config.tui.key_profile),
            theme,
        ),
        spacer_detail_item(),
        section_detail_item("navigation", theme),
    ];
    match app.ctx.config.tui.key_profile {
        TuiKeyProfile::Emacs => {
            items.push(key_value_detail_item("tabs", "←/→ or 1-6", theme));
            items.push(key_value_detail_item(
                "rows",
                "j/k, arrows, Pg, Home/End",
                theme,
            ));
        }
        TuiKeyProfile::Vim => {
            items.push(key_value_detail_item("tabs", "h/l or 1-6", theme));
            items.push(key_value_detail_item("rows", "j/k, Pg, gg/G", theme));
        }
    }
    items.push(key_value_detail_item(
        "commands",
        ": then Ctrl+n/Ctrl+p completes",
        theme,
    ));
    items
}

fn apply_environment_summary(app: &App) -> ApplyEnvironmentSummary {
    summarize_apply_environment(&app.ctx.config.apply)
}

fn apply_block_backend_summary(app: &App) -> String {
    let detection = apply_environment_summary(app);
    let configured = backend_setting_label(detection.configured_backend);
    if detection.configured_backend == ApplyBackendSetting::Auto {
        format!("{configured} → {}", detection.effective_backend_label())
    } else {
        configured.to_string()
    }
}

fn apply_display_details(app: &App) -> Vec<String> {
    let detection = apply_environment_summary(app);
    let custom_script = app
        .ctx
        .config
        .apply
        .custom_script
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .unwrap_or("(not set)");
    let mut details = vec![
        "configured (config.json):".into(),
        format!(
            "  backend: {}",
            backend_setting_label(app.ctx.config.apply.backend)
        ),
        format!("  custom script: {custom_script}"),
        format!(
            "  cosmic method: {}",
            cosmic_method_label(app.ctx.config.apply.cosmic.method)
        ),
        format!(
            "  cosmic config path: {}",
            app.ctx.config.apply.cosmic.config_path
        ),
        format!(
            "  cosmic uses original: {}",
            app.ctx.config.apply.cosmic.use_original_path
        ),
        format!("  display mode: {}", app.ctx.config.display.mode),
        format!("  EXIF auto-rotate: {}", app.ctx.config.display.auto_rotate),
        format!("  target: {}", display_target_summary(app)),
        format!(
            "  imagemagick: {}",
            app.ctx.config.display.imagemagick_command
        ),
        format!(
            "  filters: {} configured, enabled={}",
            app.ctx.config.display.filters.filters.len(),
            app.ctx.config.display.filters.enabled
        ),
        format!(
            "  filter command: {}",
            app.ctx.config.display.filters.command
        ),
        "".into(),
        "detected (this session):".into(),
    ];
    for line in detection.detection_detail_lines(app.ctx.config.apply.cosmic.method) {
        details.push(format!("  {line}"));
    }
    details.extend(config_warning_lines(app, &["apply."]));
    details
}

fn apply_display_detail_items(app: &App, theme: style::Theme) -> Vec<ListItem<'static>> {
    let detection = apply_environment_summary(app);
    let custom_script = app
        .ctx
        .config
        .apply
        .custom_script
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .unwrap_or("(not set)");
    let mut items = vec![
        section_detail_item("configured", theme),
        key_value_detail_item(
            "backend",
            backend_setting_label(app.ctx.config.apply.backend),
            theme,
        ),
        path_detail_item("custom script", custom_script, theme),
        key_value_detail_item(
            "cosmic method",
            cosmic_method_label(app.ctx.config.apply.cosmic.method),
            theme,
        ),
        path_detail_item(
            "cosmic config",
            app.ctx.config.apply.cosmic.config_path.clone(),
            theme,
        ),
        key_value_detail_item(
            "cosmic original",
            app.ctx.config.apply.cosmic.use_original_path.to_string(),
            theme,
        ),
        key_value_detail_item(
            "display mode",
            app.ctx.config.display.mode.to_string(),
            theme,
        ),
        key_value_detail_item(
            "EXIF auto-rotate",
            app.ctx.config.display.auto_rotate.to_string(),
            theme,
        ),
        key_value_detail_item("target", display_target_summary(app), theme),
        key_value_detail_item(
            "imagemagick",
            app.ctx.config.display.imagemagick_command.clone(),
            theme,
        ),
        key_value_detail_item(
            "filters",
            format!(
                "{} configured, enabled={}",
                app.ctx.config.display.filters.filters.len(),
                app.ctx.config.display.filters.enabled
            ),
            theme,
        ),
        key_value_detail_item(
            "filter command",
            app.ctx.config.display.filters.command.clone(),
            theme,
        ),
        spacer_detail_item(),
        section_detail_item("detected this session", theme),
    ];
    for line in detection.detection_detail_lines(app.ctx.config.apply.cosmic.method) {
        if let Some((label, value)) = line.split_once(": ") {
            items.push(detected_detail_item(label, value, theme));
        } else {
            items.push(ListItem::new(Line::from(vec![
                Span::raw("    · "),
                Span::styled(line, theme.muted()),
            ])));
        }
    }
    items.extend(
        config_warning_lines(app, &["apply."])
            .into_iter()
            .map(|warning| warning_detail_item(warning, theme)),
    );
    items
}

fn quota_summary(app: &App) -> String {
    if app.ctx.config.quota.enabled {
        format!("{} MB", app.ctx.config.quota.size_mb)
    } else {
        "disabled".into()
    }
}

fn display_target_summary(app: &App) -> String {
    match (
        app.ctx.config.display.target_width,
        app.ctx.config.display.target_height,
    ) {
        (Some(width), Some(height)) => format!("{width}x{height} target"),
        _ => "automatic target".into(),
    }
}

fn config_warning_lines(app: &App, prefixes: &[&str]) -> Vec<String> {
    app.config_warnings
        .iter()
        .filter(|warning| {
            prefixes
                .iter()
                .any(|prefix| warning.trim_start_matches("warning: ").starts_with(prefix))
        })
        .cloned()
        .collect()
}

fn cosmic_method_label(method: CosmicMethod) -> &'static str {
    match method {
        CosmicMethod::CosmicConfig => "cosmic-config",
        CosmicMethod::CosmicExtBgCtl => "cosmic-ext-bg-ctl",
    }
}
