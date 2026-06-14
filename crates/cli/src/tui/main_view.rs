use ratatui::prelude::*;
use ratatui::widgets::{Clear, Tabs};

use super::app::{App, Tab};
use super::layout_size::{terminal_size, TerminalSize};
#[cfg(feature = "tui-preview")]
use super::preview;
use super::{chrome_view, config_edit_view, config_view, line_view, now_view, style};

#[cfg(feature = "tui-preview")]
const PREVIEW_CONTENT_PERCENT: u16 = 55;
#[cfg(feature = "tui-preview")]
const MIN_PREVIEW_WIDTH: u16 = 48;
#[cfg(feature = "tui-preview")]
const MIN_PREVIEW_HEIGHT: u16 = 14;

#[cfg(not(feature = "tui-preview"))]
pub(crate) fn draw_inner(f: &mut Frame, app: &App) {
    let area = f.area();
    if terminal_size(area) == TerminalSize::Tiny {
        return;
    }
    let theme = style::Theme::with_preset(app.color_mode, app.ctx.config.tui.theme);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(4),
        ])
        .split(area);

    render_tabs(f, chunks[0], app, theme);
    render_tab_body(f, chunks[1], app, theme);
    render_footer(f, chunks[2], app, theme);
}

#[cfg(feature = "tui-preview")]
pub(crate) fn draw_inner(f: &mut Frame, app: &App, preview: Option<&mut preview::ImagePreview>) {
    let area = f.area();
    if terminal_size(area) == TerminalSize::Tiny {
        return;
    }
    let theme = style::Theme::with_preset(app.color_mode, app.ctx.config.tui.theme);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(4),
        ])
        .split(area);

    render_tabs(f, chunks[0], app, theme);
    render_tab_body(f, chunks[1], app, preview, theme);
    render_footer(f, chunks[2], app, theme);
}

fn render_tabs(f: &mut Frame, area: Rect, app: &App, theme: style::Theme) {
    let titles = vec!["Config", "Now", "History", "Browse", "Search", "Logs"];
    let tabs = Tabs::new(titles)
        .block(theme.chrome_block("walls"))
        .style(theme.normal())
        .highlight_style(theme.selected())
        .select(app.tab.index());
    f.render_widget(tabs, area);
}

fn render_footer(f: &mut Frame, area: Rect, app: &App, theme: style::Theme) {
    let help = chrome_view::footer_paragraph(app, area.width, theme);
    f.render_widget(help, area);
}

#[cfg(feature = "tui-preview")]
fn render_tab_body(
    f: &mut Frame,
    area: Rect,
    app: &App,
    preview: Option<&mut preview::ImagePreview>,
    theme: style::Theme,
) {
    f.render_widget(Clear, area);
    let path = selected_preview_path(app);
    if preview_split_area(area, app, path.as_deref()).is_some() {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(100 - PREVIEW_CONTENT_PERCENT),
                Constraint::Percentage(PREVIEW_CONTENT_PERCENT),
            ])
            .split(area);
        render_tab_content(f, chunks[0], app, theme, chunks[0].width);
        if let Some(preview) = preview {
            preview.render(
                f,
                chunks[1],
                path.as_deref(),
                &app.ctx.paths.cache_dir,
                theme,
            );
        } else {
            line_view::render_lines(
                f,
                chunks[1],
                "preview",
                vec!["preview unavailable".into()],
                theme,
            );
        }
    } else if app.tab == Tab::Config
        && app.is_editing()
        && terminal_size(area) == TerminalSize::Wide
    {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        line_view::render_lines(
            f,
            chunks[0],
            "List context",
            vec!["(use normal view for j/k subnav)".into()],
            theme,
        );
        config_edit_view::render_rich_edit(
            f,
            chunks[1],
            app,
            theme,
            &config_edit_view::edit_target_title(app),
        );
    } else if app.tab == Tab::Config && app.is_editing() {
        config_edit_view::render_rich_edit(
            f,
            area,
            app,
            theme,
            &config_edit_view::edit_target_title(app),
        );
    } else {
        render_tab_content(f, area, app, theme, area.width);
    }
}

#[cfg(feature = "tui-preview")]
fn preview_split_area(area: Rect, app: &App, path: Option<&str>) -> Option<Rect> {
    if app.show_key_help
        || !matches!(app.tab, Tab::Now | Tab::History | Tab::Browse | Tab::Search)
        || terminal_size(area) != TerminalSize::Wide
        || path.is_none()
    {
        return None;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(100 - PREVIEW_CONTENT_PERCENT),
            Constraint::Percentage(PREVIEW_CONTENT_PERCENT),
        ])
        .split(area);
    let preview_area = chunks[1];
    let inner_width = preview_area.width.saturating_sub(2);
    let inner_height = preview_area.height.saturating_sub(2);
    (inner_width >= MIN_PREVIEW_WIDTH && inner_height >= MIN_PREVIEW_HEIGHT).then_some(preview_area)
}

#[cfg(feature = "tui-preview")]
pub(crate) fn selected_preview_path(app: &App) -> Option<String> {
    match app.tab {
        Tab::Now => app
            .ctx
            .state
            .current
            .as_ref()
            .map(|current| current.composed_path.clone()),
        Tab::History => app
            .selected_history_preview_path()
            .map(|path| path.display().to_string()),
        Tab::Browse => app
            .selected_browse_preview_path()
            .map(|path| path.display().to_string()),
        Tab::Search => app
            .selected_search_preview_path()
            .map(|path| path.display().to_string()),
        _ => None,
    }
}

#[cfg(not(feature = "tui-preview"))]
fn render_tab_body(f: &mut Frame, area: Rect, app: &App, theme: style::Theme) {
    f.render_widget(Clear, area);
    if app.tab == Tab::Config && app.is_editing() {
        config_edit_view::render_rich_edit(
            f,
            area,
            app,
            theme,
            &config_edit_view::edit_target_title(app),
        );
    } else {
        render_tab_content(f, area, app, theme, area.width);
    }
}

fn render_tab_content(f: &mut Frame, area: Rect, app: &App, theme: style::Theme, width: u16) {
    if app.show_key_help {
        line_view::render_lines(
            f,
            area,
            "Key help",
            chrome_view::key_help_lines(app, width),
            theme,
        );
        return;
    }
    if app.tab == Tab::Config {
        config_view::render_tab(f, area, app, theme);
        return;
    }
    let (title, body) = (
        app.tab.title().to_string(),
        tab_lines(app, width, area.height),
    );
    line_view::render_lines(f, area, &title, body, theme);
}

fn tab_lines(app: &App, width: u16, height: u16) -> Vec<String> {
    match app.tab {
        Tab::Config => config_view::lines(app),
        Tab::Now => now_view::lines(app),
        Tab::History => app.history_lines(),
        Tab::Browse => app.browse_lines(),
        Tab::Search => app.search_lines(),
        Tab::Logs => app.logs_lines(width, height),
    }
}
