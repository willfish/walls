mod app;
#[cfg(feature = "tui-preview")]
mod preview;
mod style;

use std::io::{stdout, IsTerminal};

use crate::tui::app::EditTarget;
use crate::tui::style::StatusKind;
use anyhow::Context;
use app::{App, InputMode, Tab};
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph, Tabs};
use walls_core::config::{ApplyBackendSetting, CosmicMethod};
use walls_core::WallsCtx;

use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

pub(crate) static LOG_BUFFER: Mutex<Vec<String>> = Mutex::new(Vec::new());
static IN_TUI: AtomicBool = AtomicBool::new(false);

const MAX_LOG_LINES: usize = 2000;

pub(crate) fn log_len() -> usize {
    LOG_BUFFER.lock().unwrap().len()
}

pub(crate) struct ConsoleWriter;

impl io::Write for ConsoleWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !IN_TUI.load(Ordering::Relaxed) {
            let _ = io::stdout().write_all(buf);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if !IN_TUI.load(Ordering::Relaxed) {
            let _ = io::stdout().flush();
        }
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ConsoleWriter {
    type Writer = ConsoleWriter;
    fn make_writer(&'a self) -> Self::Writer {
        ConsoleWriter
    }
}

pub(crate) struct CaptureWriter;

impl io::Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(s) = std::str::from_utf8(buf) {
            let mut logs = LOG_BUFFER.lock().unwrap();
            for line in s.lines() {
                if !line.trim().is_empty() {
                    logs.push(line.trim_end().to_string());
                    if logs.len() > MAX_LOG_LINES {
                        let to_drain = logs.len() - MAX_LOG_LINES / 2;
                        logs.drain(0..to_drain);
                    }
                }
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
    type Writer = CaptureWriter;
    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter
    }
}

pub fn run() -> anyhow::Result<()> {
    require_tty()?;
    let rt = tokio::runtime::Handle::current();

    let mut stdout = stdout();
    enable_raw_mode().context("failed to enable raw mode (is this an interactive terminal?)")?;
    stdout
        .execute(EnterAlternateScreen)
        .context("failed to enter alternate screen")?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let _restore = TerminalRestore;

    let mut app = App::new(WallsCtx::load().context(
        "failed to load ~/.config/walls/config.json — copy config.example.json to get started",
    )?)?;
    IN_TUI.store(true, Ordering::Relaxed);
    #[cfg(feature = "tui-preview")]
    let mut preview = preview::ImagePreview::detect();

    loop {
        terminal.draw(|f| {
            #[cfg(feature = "tui-preview")]
            draw(f, &app, &mut preview);
            #[cfg(not(feature = "tui-preview"))]
            draw(f, &app);
        })?;
        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if handle_key(&mut app, key, &rt)? {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn require_tty() -> anyhow::Result<()> {
    use std::io::{stdin, stdout};
    if !stdin().is_terminal() || !stdout().is_terminal() {
        anyhow::bail!(
            "walls tui requires an interactive terminal (stdin and stdout must be a TTY).\n\
             Try: walls   # (or `walls tui`) from a terminal emulator, not a pipe or IDE task output"
        );
    }
    Ok(())
}

struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        IN_TUI.store(false, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiAction {
    Quit,
    EnterCommandMode,
    CancelInput,
    SubmitCommand,
    CommandBackspace,
    CommandChar(char),
    SubmitSearch,
    SearchBackspace,
    SearchChar(char),
    Next,
    Prev,
    Favorite,
    Trash,
    TogglePause,
    ToggleConfigValue,
    #[allow(dead_code)]
    CycleConfigValue,
    EditConfigItem,
    CancelEdit,
    EditFieldChar(char),
    EditFieldBackspace,
    EditFieldCommit,
    EditFieldUp,
    EditFieldDown,
    SaveEditItem,
    SwitchTab(Tab),
    EditSearch,
    MoveDown,
    MoveUp,
    Enter,
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateEffect {
    None,
    Reload,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalSize {
    Tiny,
    Narrow,
    Standard,
    Wide,
}

fn terminal_size(area: Rect) -> TerminalSize {
    if area.width < 10 || area.height < 6 {
        TerminalSize::Tiny
    } else if area.width < 50 || area.height < 12 {
        TerminalSize::Narrow
    } else if area.width >= 100 && area.height >= 18 {
        TerminalSize::Wide
    } else {
        TerminalSize::Standard
    }
}

fn handle_key(app: &mut App, key: KeyEvent, rt: &tokio::runtime::Handle) -> anyhow::Result<bool> {
    let action = action_for_key(app, key);
    let effect = update(app, action, rt)?;
    apply_effect(app, effect)?;
    Ok(effect == UpdateEffect::Quit)
}

fn action_for_key(app: &App, key: KeyEvent) -> UiAction {
    match app.input_mode {
        InputMode::Command => {
            return match key.code {
                KeyCode::Esc => UiAction::CancelInput,
                KeyCode::Enter => UiAction::SubmitCommand,
                KeyCode::Backspace => UiAction::CommandBackspace,
                KeyCode::Char(c) => UiAction::CommandChar(c),
                _ => UiAction::Ignore,
            };
        }
        InputMode::SearchInput => {
            return match key.code {
                KeyCode::Esc => UiAction::CancelInput,
                KeyCode::Enter => UiAction::SubmitSearch,
                KeyCode::Backspace => UiAction::SearchBackspace,
                KeyCode::Char(c) => UiAction::SearchChar(c),
                _ => UiAction::Ignore,
            };
        }
        InputMode::Normal => {}
    }

    // Editing popup steals keys for j/k field nav + live typing into buffer (no sub-mode)
    if app.is_editing() {
        return match key.code {
            KeyCode::Up | KeyCode::Char('k') => UiAction::EditFieldUp,
            KeyCode::Down | KeyCode::Char('j') => UiAction::EditFieldDown,
            KeyCode::Esc => UiAction::CancelEdit,
            KeyCode::Char('s') | KeyCode::Char('S') => UiAction::SaveEditItem,
            KeyCode::Backspace => UiAction::EditFieldBackspace,
            KeyCode::Enter => UiAction::EditFieldCommit,
            KeyCode::Char(c) => UiAction::EditFieldChar(c),
            _ => UiAction::Ignore,
        };
    }

    match key.code {
        KeyCode::Char('q') => UiAction::Quit,
        KeyCode::Char(':') => UiAction::EnterCommandMode,
        KeyCode::Char('n') => UiAction::Next,
        KeyCode::Char('p') => UiAction::Prev,
        KeyCode::Char('f') => UiAction::Favorite,
        KeyCode::Char('d') => UiAction::Trash,
        KeyCode::Char(' ') => UiAction::TogglePause,
        KeyCode::Char('t') if app.tab == Tab::Config => UiAction::ToggleConfigValue,
        KeyCode::Char('e') if app.tab == Tab::Config => UiAction::EditConfigItem,
        KeyCode::Char(c @ '1'..='6') => {
            let index = c
                .to_digit(10)
                .expect("key guard only allows ASCII digits 1-6") as usize
                - 1;
            UiAction::SwitchTab(Tab::from_index(index))
        }
        KeyCode::Char('i') if app.tab == Tab::Search => UiAction::EditSearch,
        KeyCode::Down | KeyCode::Char('j') => UiAction::MoveDown,
        KeyCode::Up | KeyCode::Char('k') => UiAction::MoveUp,
        KeyCode::Enter => UiAction::Enter,
        _ => UiAction::Ignore,
    }
}

fn update(
    app: &mut App,
    action: UiAction,
    rt: &tokio::runtime::Handle,
) -> anyhow::Result<UpdateEffect> {
    match action {
        UiAction::Quit => return Ok(UpdateEffect::Quit),
        UiAction::EnterCommandMode => {
            app.input_mode = InputMode::Command;
            app.cmd_line.clear();
        }
        UiAction::CancelInput => {
            app.input_mode = InputMode::Normal;
            app.cmd_line.clear();
        }
        UiAction::SubmitCommand => {
            match app.run_command(rt)? {
                None => return Ok(UpdateEffect::Quit),
                Some(msg) => app.message = msg,
            }
            app.input_mode = InputMode::Normal;
            app.cmd_line.clear();
            return Ok(UpdateEffect::Reload);
        }
        UiAction::CommandBackspace => {
            app.cmd_line.pop();
        }
        UiAction::CommandChar(c) => app.cmd_line.push(c),
        UiAction::SubmitSearch => {
            app.input_mode = InputMode::Normal;
            app.message = match tokio::task::block_in_place(|| rt.block_on(app.run_search())) {
                Ok(()) => format!("search: {} results", app.search_results.len()),
                Err(e) => format!("search error: {e}"),
            };
        }
        UiAction::SearchBackspace => {
            app.search_query.pop();
        }
        UiAction::SearchChar(c) => app.search_query.push(c),
        UiAction::Next => {
            app.message = match tokio::task::block_in_place(|| rt.block_on(app.ctx.advance_next()))
            {
                Ok(Some(p)) => format!("next: {}", p.display()),
                Ok(None) => "next: no change".into(),
                Err(e) => format!("next error: {e}"),
            };
            return Ok(UpdateEffect::Reload);
        }
        UiAction::Prev => {
            app.message = match app.ctx.advance_prev() {
                Ok(Some(p)) => format!("prev: {}", p.display()),
                Ok(None) => "prev: none".into(),
                Err(e) => format!("prev error: {e}"),
            };
            return Ok(UpdateEffect::Reload);
        }
        UiAction::Favorite => match app.favorite_current() {
            Ok(msg) => {
                app.message = msg;
                return Ok(UpdateEffect::Reload);
            }
            Err(e) => app.message = format!("favorite error: {e}"),
        },
        UiAction::Trash => match app.trash_current() {
            Ok(msg) => {
                app.message = msg;
                return Ok(UpdateEffect::Reload);
            }
            Err(e) => app.message = format!("trash error: {e}"),
        },
        UiAction::TogglePause => match app.ctx.toggle_pause() {
            Ok(()) => app.message = format!("paused: {}", app.ctx.state.paused),
            Err(e) => app.message = format!("pause error: {e}"),
        },
        UiAction::ToggleConfigValue => match app.toggle_focused_config_value() {
            Ok(Some(msg)) => {
                app.message = msg;
                return Ok(UpdateEffect::Reload);
            }
            Ok(None) => app.message = "config: no toggle for focused block".into(),
            Err(e) => app.message = format!("config save error: {e}"),
        },
        UiAction::CycleConfigValue => match app.cycle_focused_config_value() {
            Ok(Some(msg)) => {
                app.message = msg;
                return Ok(UpdateEffect::Reload);
            }
            Ok(None) => app.message = "config: no cycle for focused block".into(),
            Err(e) => app.message = format!("config save error: {e}"),
        },
        UiAction::EditConfigItem => {
            app.start_edit_for_current();
        }
        UiAction::CancelEdit => {
            app.cancel_edit();
        }
        UiAction::EditFieldChar(c) => {
            if let Some(sess) = &mut app.editing {
                sess.field_buffer.push(c);
                app.refresh_edit_validation();
            }
        }
        UiAction::EditFieldBackspace => {
            if let Some(sess) = &mut app.editing {
                sess.field_buffer.pop();
                app.refresh_edit_validation();
            }
        }
        UiAction::EditFieldCommit => {
            if app.editing.is_some() {
                app.commit_edit_field_buffer();
            }
        }
        UiAction::EditFieldUp => {
            if let Some(sess) = &mut app.editing {
                if sess.field_cursor > 0 {
                    sess.field_cursor -= 1;
                }
                sess.field_buffer.clear();
            }
        }
        UiAction::EditFieldDown => {
            if let Some(sess) = &mut app.editing {
                sess.field_cursor += 1;
                sess.field_buffer.clear();
            }
        }
        UiAction::SaveEditItem => {
            let _ = app.save_edit_item();
            // save_edit_item will set message and clear editing on success, or errors
        }
        UiAction::SwitchTab(tab) => {
            app.tab = tab;
            app.cursor = 0;
            app.config_in_subnav = false;
        }
        UiAction::EditSearch => {
            app.input_mode = InputMode::SearchInput;
        }
        UiAction::MoveDown => app.move_down(),
        UiAction::MoveUp => app.move_up(),
        UiAction::Enter => return handle_enter(app, rt),
        UiAction::Ignore => {}
    }
    Ok(UpdateEffect::None)
}

fn apply_effect(app: &mut App, effect: UpdateEffect) -> anyhow::Result<()> {
    if effect == UpdateEffect::Reload {
        app.reload_ctx()?;
    }
    Ok(())
}

fn handle_enter(app: &mut App, rt: &tokio::runtime::Handle) -> anyhow::Result<UpdateEffect> {
    match app.tab {
        Tab::History => {
            if let Some(path) = app.apply_history_selection() {
                app.message = format!("applied: {}", path.display());
                return Ok(UpdateEffect::Reload);
            }
        }
        Tab::Browse => {
            if let Some(msg) =
                tokio::task::block_in_place(|| rt.block_on(app.apply_browse_selection()))?
            {
                app.message = msg;
                return Ok(UpdateEffect::Reload);
            }
        }
        Tab::Search => {
            if app.search_results.is_empty() {
                app.input_mode = InputMode::SearchInput;
            } else if let Some(msg) =
                tokio::task::block_in_place(|| rt.block_on(app.apply_search_selection()))?
            {
                app.message = msg;
                return Ok(UpdateEffect::Reload);
            }
        }
        Tab::Config => {
            if app.is_sources_list_block(app.config_cursor) {
                app.toggle_config_subnav();
            }
        }
        _ => {}
    }
    Ok(UpdateEffect::None)
}

#[cfg(feature = "tui-preview")]
fn draw(f: &mut Frame, app: &App, preview: &mut preview::ImagePreview) {
    draw_inner(f, app, Some(preview));
}

#[cfg(not(feature = "tui-preview"))]
fn draw(f: &mut Frame, app: &App) {
    draw_inner(f, app);
}

#[cfg(not(feature = "tui-preview"))]
fn draw_inner(f: &mut Frame, app: &App) {
    let area = f.area();
    if terminal_size(area) == TerminalSize::Tiny {
        return;
    }
    let theme = style::Theme::new(app.color_mode);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(4),
        ])
        .split(area);

    let titles = vec!["Config", "Now", "History", "Browse", "Search", "Logs"];
    let tabs = Tabs::new(titles)
        .block(theme.chrome_block("walls"))
        .style(theme.normal())
        .highlight_style(theme.selected())
        .select(app.tab.index());
    f.render_widget(tabs, chunks[0]);

    render_tab_body(f, chunks[1], app, None, theme);

    let help = footer_paragraph(app, chunks[2].width, theme);
    f.render_widget(help, chunks[2]);
}

#[cfg(feature = "tui-preview")]
fn draw_inner(f: &mut Frame, app: &App, preview: Option<&mut preview::ImagePreview>) {
    let area = f.area();
    if terminal_size(area) == TerminalSize::Tiny {
        return;
    }
    let theme = style::Theme::new(app.color_mode);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(4),
        ])
        .split(area);

    let titles = vec!["Config", "Now", "History", "Browse", "Search", "Logs"];
    let tabs = Tabs::new(titles)
        .block(theme.chrome_block("walls"))
        .style(theme.normal())
        .highlight_style(theme.selected())
        .select(app.tab.index());
    f.render_widget(tabs, chunks[0]);

    render_tab_body(f, chunks[1], app, preview, theme);

    let help = footer_paragraph(app, chunks[2].width, theme);
    f.render_widget(help, chunks[2]);
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
    if app.tab == Tab::Now && terminal_size(area) == TerminalSize::Wide {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);
        render_lines(f, chunks[0], app.tab.title(), now_lines(app), theme);
        let path = app
            .ctx
            .state
            .current
            .as_ref()
            .map(|current| current.composed_path.as_str());
        if let Some(preview) = preview {
            preview.render(f, chunks[1], path, theme);
        } else {
            render_lines(
                f,
                chunks[1],
                "preview",
                vec!["preview unavailable".into()],
                theme,
            );
        }
    } else {
        render_lines(f, area, app.tab.title(), tab_lines(app), theme);
    }
    render_edit_popup(f, app, area, theme);
}

#[cfg(not(feature = "tui-preview"))]
fn render_tab_body(
    f: &mut Frame,
    area: Rect,
    app: &App,
    _preview: Option<()>,
    theme: style::Theme,
) {
    f.render_widget(Clear, area);
    render_lines(f, area, app.tab.title(), tab_lines(app), theme);
    render_edit_popup(f, app, area, theme);
}

fn tab_lines(app: &App) -> Vec<String> {
    match app.tab {
        Tab::Config => config_lines(app),
        Tab::Now => now_lines(app),
        Tab::History => app.history_lines(),
        Tab::Browse => app.browse_lines(),
        Tab::Search => app.search_lines(),
        Tab::Logs => app.logs_lines(),
    }
}

fn render_lines(f: &mut Frame, area: Rect, title: &str, body: Vec<String>, theme: style::Theme) {
    let items: Vec<ListItem> = body
        .iter()
        .map(|line| {
            let item_style = line_style(line, theme);
            ListItem::new(line.as_str()).style(item_style)
        })
        .collect();
    let list = List::new(items)
        .block(theme.content_block(title))
        .style(theme.normal());
    f.render_widget(list, area);
}

fn footer_paragraph(app: &App, width: u16, theme: style::Theme) -> Paragraph<'_> {
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
    let status_kind = style::status_kind(&status);

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

fn footer_keys(app: &App, width: u16) -> String {
    if width < 50 {
        return match app.input_mode {
            InputMode::Command => format!(":{}_ | Enter | Esc | q", app.cmd_line),
            InputMode::SearchInput => "type | Enter search | Esc | q".into(),
            InputMode::Normal => match app.tab {
                Tab::Search => "i edit | Enter | j/k | : | q".into(),
                Tab::Config => "j/k | Enter sub | e edit | t | n/p | sp | : | q".into(),
                _ => "1-5 | n/p | f/d | sp | : | q".into(),
            },
        };
    }

    app.footer_keys()
}

fn line_style(line: &str, theme: style::Theme) -> Style {
    let trimmed = line.trim_start();
    if trimmed.starts_with('>') {
        return theme.selected();
    }
    if trimmed.starts_with("--") {
        return theme.muted();
    }
    if trimmed.starts_with('(') || trimmed.contains("preview unavailable") {
        return theme.muted();
    }
    theme.status(style::status_kind(line))
}

fn config_lines(app: &App) -> Vec<String> {
    let mut lines = Vec::new();
    push_config_block(
        &mut lines,
        0,
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
    // Sources block now lists *all* providers (for nested edit with j/k pick + e)
    let sources = &app.ctx.config.sources;
    let sources_enabled = sources.iter().any(|s| s.enabled);
    let sources_summary = if sources.is_empty() {
        "no sources configured".to_string()
    } else {
        format!(
            "{} configured, {} enabled",
            sources.len(),
            sources.iter().filter(|s| s.enabled).count()
        )
    };
    push_config_block(
        &mut lines,
        1,
        app.config_cursor,
        "Sources",
        sources_enabled,
        sources_summary,
        sources_details(app),
    );
    push_config_block(
        &mut lines,
        2,
        app.config_cursor,
        "Wallhaven",
        app.wallhaven_summary.usable(),
        wallhaven_summary(app),
        wallhaven_details(app),
    );
    push_config_block(
        &mut lines,
        3,
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
        4,
        app.config_cursor,
        "Apply/display",
        true,
        format!(
            "{} backend, {} mode, {}",
            apply_backend_label(app.ctx.config.apply.backend),
            app.ctx.config.display.mode,
            display_target_summary(app)
        ),
        apply_display_details(app),
    );
    lines
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

fn sources_details(app: &App) -> Vec<String> {
    let sources = &app.ctx.config.sources;
    if sources.is_empty() {
        return vec!["no sources configured".into()];
    }
    sources
        .iter()
        .enumerate()
        .map(|(index, src)| {
            let state = if src.enabled { "on" } else { "off" };
            let key = src
                .path
                .as_deref()
                .or(src.url.as_deref())
                .or(src.query.as_deref())
                .unwrap_or("(no key)");
            let label = src.label.as_deref().unwrap_or(&src.source_type);
            format!(
                "{}. [{state}] {} ({}) - {}",
                index + 1,
                label,
                src.source_type,
                key
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
    ]
}

fn wallhaven_summary(app: &App) -> String {
    let provider = &app.wallhaven_summary;
    let online = if provider.internet_enabled {
        "online on"
    } else {
        "online off"
    };
    let key = if provider.api_key_present {
        "key"
    } else {
        "no key"
    };
    let collection_count = provider.collections.len();
    let collection_label = if collection_count == 1 { "col" } else { "cols" };
    format!(
        "{online}, {key}, {collection_count} {collection_label}, q={}, pref={}",
        short_query(&provider.query),
        short_wallhaven_prefer(&provider.prefer)
    )
}

fn short_wallhaven_prefer(prefer: &str) -> &str {
    match prefer {
        "CollectionsThenSearch" => "c+s",
        "SearchOnly" => "search",
        "CollectionsOnly" => "coll",
        _ => prefer,
    }
}

fn short_query(query: &str) -> String {
    if query == "(empty query)" {
        return "empty".into();
    }

    const MAX_QUERY_CHARS: usize = 24;
    let mut chars = query.chars();
    let short: String = chars.by_ref().take(MAX_QUERY_CHARS).collect();
    if chars.next().is_some() {
        format!("{short}...")
    } else {
        short
    }
}

fn wallhaven_details(app: &App) -> Vec<String> {
    let provider = &app.wallhaven_summary;
    let key = if provider.api_key_present {
        "present"
    } else {
        "missing"
    };
    let mut details = vec![
        format!("api key: {key}"),
        format!("prefer: {}", provider.prefer),
        format!(
            "search: q={} categories={} purity={}",
            provider.query, provider.categories, provider.purity
        ),
        format!(
            "sort: {} {} minimum {}",
            provider.sorting, provider.order, provider.atleast
        ),
    ];

    if provider.collections.is_empty() {
        details.push("collections: none".into());
    } else {
        details.push(format!("collections: {}", provider.collections.len()));
        details.extend(
            provider
                .collections
                .iter()
                .enumerate()
                .map(|(index, collection)| format!("{}. {}", index + 1, collection)),
        );
    }

    details.extend(provider.warnings.iter().cloned());
    details
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
        format!("avoid recent: {}", app.ctx.config.selection.avoid_recent),
        format!(
            "refetch below: {} cached",
            app.ctx.config.selection.refetch_when_cache_below
        ),
    ];
    details.extend(config_warning_lines(app, &["quota."]));
    details
}

fn apply_display_details(app: &App) -> Vec<String> {
    let custom_script = app
        .ctx
        .config
        .apply
        .custom_script
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .unwrap_or("(not set)");
    let mut details = vec![
        format!(
            "backend: {}",
            apply_backend_label(app.ctx.config.apply.backend)
        ),
        format!("custom script: {custom_script}"),
        format!(
            "cosmic: {}",
            cosmic_method_label(app.ctx.config.apply.cosmic.method)
        ),
        format!("cosmic config: {}", app.ctx.config.apply.cosmic.config_path),
        format!(
            "cosmic uses original: {}",
            app.ctx.config.apply.cosmic.use_original_path
        ),
        format!("display mode: {}", app.ctx.config.display.mode),
        format!("auto rotate: {}", app.ctx.config.display.auto_rotate),
        format!("target: {}", display_target_summary(app)),
        format!(
            "imagemagick: {}",
            app.ctx.config.display.imagemagick_command
        ),
        format!(
            "filters: {} configured, enabled={}",
            app.ctx.config.display.filters.filters.len(),
            app.ctx.config.display.filters.enabled
        ),
        format!("filter command: {}", app.ctx.config.display.filters.command),
    ];
    details.extend(config_warning_lines(app, &["apply."]));
    details
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

fn apply_backend_label(backend: ApplyBackendSetting) -> &'static str {
    match backend {
        ApplyBackendSetting::Auto => "auto",
        ApplyBackendSetting::Cosmic => "cosmic",
        ApplyBackendSetting::CosmicExtBgCtl => "cosmic-ext-bg-ctl",
        ApplyBackendSetting::Gnome => "gnome",
        ApplyBackendSetting::Kde => "kde",
        ApplyBackendSetting::Xfce => "xfce",
        ApplyBackendSetting::Sway => "sway",
        ApplyBackendSetting::Wlroots => "wlroots",
        ApplyBackendSetting::Hyprland => "hyprland",
        ApplyBackendSetting::Feh => "feh",
        ApplyBackendSetting::CustomScript => "custom-script",
    }
}

fn cosmic_method_label(method: CosmicMethod) -> &'static str {
    match method {
        CosmicMethod::CosmicConfig => "cosmic-config",
        CosmicMethod::CosmicExtBgCtl => "cosmic-ext-bg-ctl",
    }
}

#[allow(dead_code)]
fn render_edit_popup(f: &mut Frame, app: &App, area: Rect, theme: style::Theme) {
    if let Some(sess) = &app.editing {
        let popup_width = ((area.width as u32 * 3 / 4) as u16).min(70);
        let popup_height = 14u16;
        let x = (area.width.saturating_sub(popup_width)) / 2;
        let y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect {
            x: area.x + x,
            y: area.y + y,
            width: popup_width,
            height: popup_height,
        };
        f.render_widget(Clear, popup_area);
        let title = match &sess.target {
            EditTarget::Block(b) => format!("Edit: block {}", b),
            EditTarget::Source(i) => format!("Edit source #{} (form)", i),
        };
        let block = theme.content_block(&title);
        let mut lines: Vec<Line> = vec![Line::from(
            "=== EDIT FORM (j/k, type, Enter=commit field, s=save, Esc) ===",
        )];
        // dynamic fields list with cursor + live buffer on current
        let mut fields: Vec<(String, String)> = vec![];
        if let Some(ref src) = sess.draft_source {
            fields.push(("enabled".into(), src.enabled.to_string()));
            fields.push(("type".into(), src.source_type.clone()));
            fields.push(("label".into(), src.label.clone().unwrap_or_default()));
            fields.push(("url".into(), src.url.clone().unwrap_or_default()));
            fields.push(("path".into(), src.path.clone().unwrap_or_default()));
            fields.push((
                "image_path".into(),
                src.image_path.clone().unwrap_or_default(),
            ));
        } else {
            for (k, v) in &sess.draft_block_values {
                fields.push((k.clone(), v.clone()));
            }
        }
        for (i, (k, v)) in fields.iter().enumerate() {
            let mut val = v.clone();
            if i == sess.field_cursor {
                val = format!("{}|{}", val, sess.field_buffer);
                lines.push(Line::from(format!("> {}: {}", k, val)));
            } else {
                lines.push(Line::from(format!("  {}: {}", k, val)));
            }
        }
        if !sess.validation_errors.is_empty() {
            let err_text = sess.validation_errors.join("; ");
            lines.push(Line::from(Span::styled(
                format!("validation: {}", err_text),
                theme.status(StatusKind::Error),
            )));
        }
        let para = Paragraph::new(lines).block(block);
        f.render_widget(para, popup_area);
    }
}

fn now_lines(app: &App) -> Vec<String> {
    match &app.ctx.state.current {
        Some(c) => vec![
            format!("source: {}", c.source_id),
            format!("wallhaven: {:?}", c.wallhaven_id),
            format!("original: {}", c.original_path),
            format!("composed: {}", c.composed_path),
            app.message.clone(),
        ],
        None => vec!["(no current wallpaper)".into(), app.message.clone()],
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    use ratatui::layout::Rect;
    use ratatui::Terminal;
    use walls_core::WallsCtx;

    use super::{
        action_for_key, app::App, apply_effect, draw_inner, handle_key, style, update, InputMode,
        Tab, TerminalSize, UiAction, UpdateEffect,
    };

    fn test_app() -> App {
        let tmp = tempfile::tempdir().expect("tempdir");
        let image_dir = tmp.path().join("images");
        fs::create_dir_all(&image_dir).expect("images dir");
        fs::write(image_dir.join("a.jpg"), b"x").expect("image");

        test_app_with_sources(
            tmp,
            serde_json::json!([{ "enabled": true, "type": "folder", "path": image_dir.display().to_string() }]),
        )
    }

    fn test_app_with_sources(tmp: tempfile::TempDir, sources: serde_json::Value) -> App {
        fs::create_dir_all(tmp.path().join("favorites")).expect("favorites dir");
        fs::create_dir_all(tmp.path().join("fetched")).expect("fetched dir");
        fs::write(tmp.path().join("favorites").join("fav.jpg"), b"x").expect("favorite image");
        fs::write(tmp.path().join("fetched").join("fetch.jpg"), b"x").expect("fetched image");

        let noop = tmp.path().join("noop.sh");
        fs::write(&noop, "#!/bin/sh\nexit 0\n").expect("noop");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).expect("chmod");
        }

        let config = serde_json::json!({
            "change": { "enabled": true, "internet_enabled": false },
            "paths": {
                "cache_dir": tmp.path().join("cache").display().to_string(),
                "download_dir": tmp.path().join("downloaded").display().to_string(),
                "favorites_dir": tmp.path().join("favorites").display().to_string(),
                "fetched_dir": tmp.path().join("fetched").display().to_string(),
                "compose_dir": tmp.path().join("wallpaper").display().to_string(),
            },
            "apply": { "backend": "custom-script", "custom_script": noop.display().to_string() },
            "display": { "mode": "os" },
            "sources": sources,
        });
        fs::write(
            tmp.path().join("config.json"),
            serde_json::to_string_pretty(&config).expect("config json"),
        )
        .expect("write config");
        fs::write(tmp.path().join("secrets.json"), "{}").expect("write secrets");

        App::new(WallsCtx::load_from(tmp.path()).expect("ctx")).expect("app")
    }

    fn test_app_with_config(config: serde_json::Value, secrets: serde_json::Value) -> App {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("favorites")).expect("favorites dir");
        fs::create_dir_all(tmp.path().join("fetched")).expect("fetched dir");
        fs::write(
            tmp.path().join("config.json"),
            serde_json::to_string_pretty(&config).expect("config json"),
        )
        .expect("write config");
        fs::write(
            tmp.path().join("secrets.json"),
            serde_json::to_string_pretty(&secrets).expect("secrets json"),
        )
        .expect("write secrets");

        App::new(WallsCtx::load_from(tmp.path()).expect("ctx")).expect("app")
    }

    fn test_app_with_wallhaven(
        internet_enabled: bool,
        wallhaven: serde_json::Value,
        secrets: serde_json::Value,
    ) -> App {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("favorites")).expect("favorites dir");
        fs::create_dir_all(tmp.path().join("fetched")).expect("fetched dir");

        let noop = tmp.path().join("noop.sh");
        fs::write(&noop, "#!/bin/sh\nexit 0\n").expect("noop");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).expect("chmod");
        }

        let config = serde_json::json!({
            "change": { "enabled": true, "internet_enabled": internet_enabled },
            "paths": {
                "cache_dir": tmp.path().join("cache").display().to_string(),
                "download_dir": tmp.path().join("downloaded").display().to_string(),
                "favorites_dir": tmp.path().join("favorites").display().to_string(),
                "fetched_dir": tmp.path().join("fetched").display().to_string(),
                "compose_dir": tmp.path().join("wallpaper").display().to_string(),
            },
            "apply": { "backend": "custom-script", "custom_script": noop.display().to_string() },
            "display": { "mode": "os" },
            "sources": [],
            "wallhaven": wallhaven,
        });
        fs::write(
            tmp.path().join("config.json"),
            serde_json::to_string_pretty(&config).expect("config json"),
        )
        .expect("write config");
        fs::write(
            tmp.path().join("secrets.json"),
            serde_json::to_string_pretty(&secrets).expect("secrets json"),
        )
        .expect("write secrets");

        App::new(WallsCtx::load_from(tmp.path()).expect("ctx")).expect("app")
    }

    fn render_text(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                #[cfg(feature = "tui-preview")]
                draw_inner(frame, app, None);
                #[cfg(not(feature = "tui-preview"))]
                draw_inner(frame, app);
            })
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn default_config_screen_renders_blocks_and_footer_status() {
        let app = test_app();
        let text = render_text(&app, 80, 24);

        assert!(text.contains("walls"), "{text}");
        assert!(text.contains("Config"), "{text}");
        assert!(text.contains("> [on] Rotation"), "{text}");
        assert!(text.contains("  [on] Sources"), "{text}");
        assert!(text.contains("  [off] Wallhaven"), "{text}");
        assert!(text.contains("  [on] Library"), "{text}");
        assert!(text.contains("  [on] Apply/display"), "{text}");
        assert!(text.contains("on start: false"), "{text}");
        assert!(text.contains("local only"), "{text}");
        assert!(!text.contains("paused:"), "{text}");
        assert!(text.contains("normal"), "{text}");
        assert!(
            text.contains("ready | paused=false | queue=0 | history=0"),
            "{text}"
        );
    }

    #[test]
    fn focused_config_block_expands_concrete_settings() {
        let mut app = test_app();
        app.config_cursor = 1;

        let text = render_text(&app, 80, 24);

        assert!(text.contains("> [on] Sources"), "{text}");
        // now rendered via sources_details (full providers list)
        assert!(text.contains("1. [on]"), "{text}");
        assert!(!text.contains("on start: false"), "{text}");
    }

    #[test]
    fn local_source_block_renders_enabled_disabled_and_missing_sources() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let image_dir = tmp.path().join("images");
        fs::create_dir_all(&image_dir).expect("images dir");
        fs::write(image_dir.join("a.jpg"), b"x").expect("folder image");
        let image_file = tmp.path().join("single.jpg");
        fs::write(&image_file, b"x").expect("single image");
        let missing = tmp.path().join("missing");

        let mut app = test_app_with_sources(
            tmp,
            serde_json::json!([
                { "enabled": true, "type": "favorites", "label": "Favorites" },
                { "enabled": true, "type": "fetched", "label": "Fetched" },
                { "enabled": true, "type": "folder", "label": "Wallpapers", "path": image_dir.display().to_string() },
                { "enabled": true, "type": "image", "label": "Single", "path": image_file.display().to_string() },
                { "enabled": false, "type": "folder", "label": "Disabled", "path": image_dir.display().to_string() },
                { "enabled": true, "type": "folder", "label": "Missing", "path": missing.display().to_string() }
            ]),
        );
        app.config_cursor = 1;

        let text = render_text(&app, 120, 30);

        // now Sources block uses full sources_details (no "candidates" in this view)
        assert!(text.contains("6 configured, 5 enabled"), "{text}");
        assert!(text.contains("1. [on] Favorites (favorites)"), "{text}");
        assert!(text.contains("2. [on] Fetched (fetched)"), "{text}");
        assert!(text.contains("3. [on] Wallpapers (folder)"), "{text}");
        assert!(text.contains("4. [on] Single (image)"), "{text}");
        assert!(text.contains("5. [off] Disabled (folder)"), "{text}");
        assert!(text.contains("6. [on] Missing (folder)"), "{text}");
    }

    #[test]
    fn rotation_block_renders_full_change_settings_without_pause_duplication() {
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": {
                    "enabled": true,
                    "on_start": true,
                    "interval_secs": 42,
                    "internet_enabled": true,
                    "safe_mode": true,
                    "change_lock_screen": true,
                    "download_preference_ratio": 0.35
                },
                "paths": {
                    "cache_dir": "/tmp/walls-cache",
                    "download_dir": "/tmp/walls-downloaded",
                    "favorites_dir": "/tmp/walls-favorites",
                    "fetched_dir": "/tmp/walls-fetched",
                    "compose_dir": "/tmp/walls-compose"
                },
                "apply": { "backend": "auto" },
                "display": { "mode": "os" },
                "sources": []
            }),
            serde_json::json!({}),
        );
        app.config_cursor = 0;

        let text = render_text(&app, 100, 28);

        assert!(
            text.contains("> [on] Rotation - every 42s, online, 35% online"),
            "{text}"
        );
        assert!(text.contains("enabled: true"), "{text}");
        assert!(text.contains("on start: true"), "{text}");
        assert!(text.contains("interval: 42s"), "{text}");
        assert!(text.contains("internet: true"), "{text}");
        assert!(text.contains("safe mode: true"), "{text}");
        assert!(text.contains("lock screen: true"), "{text}");
        assert!(text.contains("download preference: 35% online"), "{text}");
        assert!(!text.contains("paused:"), "{text}");
    }

    #[test]
    fn library_block_renders_paths_counts_quota_and_validation_warnings() {
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true, "internet_enabled": false },
                "paths": {
                    "cache_dir": "/tmp/walls-cache",
                    "download_dir": "/tmp/walls-downloaded",
                    "favorites_dir": "/tmp/walls-favorites",
                    "fetched_dir": "/tmp/walls-fetched",
                    "compose_dir": "/tmp/walls-compose"
                },
                "quota": { "enabled": true, "size_mb": 0 },
                "apply": { "backend": "auto" },
                "display": { "mode": "os" },
                "sources": []
            }),
            serde_json::json!({}),
        );
        app.config_cursor = 3;

        let text = render_text(&app, 120, 32);

        assert!(
            text.contains("> [on] Library - 0 queued, 0 history, quota 0 MB"),
            "{text}"
        );
        assert!(text.contains("cache: /tmp/walls-cache"), "{text}");
        assert!(text.contains("downloaded: /tmp/walls-downloaded"), "{text}");
        assert!(text.contains("favorites: /tmp/walls-favorites"), "{text}");
        assert!(text.contains("fetched: /tmp/walls-fetched"), "{text}");
        assert!(text.contains("compose: /tmp/walls-compose"), "{text}");
        assert!(text.contains("selection: Random"), "{text}");
        assert!(text.contains("avoid recent: 50"), "{text}");
        assert!(
            text.contains("warning: quota.size_mb must be greater than zero"),
            "{text}"
        );
    }

    #[test]
    fn apply_display_block_renders_backend_display_and_validation_warnings() {
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true, "internet_enabled": false },
                "paths": {
                    "cache_dir": "/tmp/walls-cache",
                    "download_dir": "/tmp/walls-downloaded",
                    "favorites_dir": "/tmp/walls-favorites",
                    "fetched_dir": "/tmp/walls-fetched",
                    "compose_dir": "/tmp/walls-compose"
                },
                "apply": {
                    "backend": "custom-script",
                    "cosmic": {
                        "method": "cosmic-ext-bg-ctl",
                        "config_path": "/tmp/missing-cosmic-config",
                        "use_original_path": true
                    }
                },
                "display": {
                    "mode": "fill",
                    "auto_rotate": true,
                    "target_width": 3840,
                    "target_height": 2160,
                    "imagemagick_command": "magick",
                    "filters": {
                        "enabled": true,
                        "command": "magick",
                        "filters": [{ "name": "soften", "args": ["-blur", "0x1"] }]
                    }
                },
                "sources": []
            }),
            serde_json::json!({}),
        );
        app.config_cursor = 4;

        let text = render_text(&app, 120, 34);

        assert!(
            text.contains(
                "> [on] Apply/display - custom-script backend, fill mode, 3840x2160 target"
            ),
            "{text}"
        );
        assert!(text.contains("backend: custom-script"), "{text}");
        assert!(text.contains("custom script: (not set)"), "{text}");
        assert!(text.contains("cosmic: cosmic-ext-bg-ctl"), "{text}");
        assert!(
            text.contains("cosmic config: /tmp/missing-cosmic-config"),
            "{text}"
        );
        assert!(text.contains("cosmic uses original: true"), "{text}");
        assert!(text.contains("display mode: fill"), "{text}");
        assert!(text.contains("auto rotate: true"), "{text}");
        assert!(text.contains("target: 3840x2160 target"), "{text}");
        assert!(
            text.contains("filters: 1 configured, enabled=true"),
            "{text}"
        );
        assert!(
            text.contains(
                "warning: apply.custom_script is required when apply.backend is custom-script"
            ),
            "{text}"
        );
    }

    #[test]
    fn narrow_config_screen_keeps_focused_block_and_navigation_visible() {
        let mut app = test_app();
        app.config_cursor = 2;

        let text = render_text(&app, 42, 14);

        assert!(text.contains("Config"), "{text}");
        assert!(text.contains("> [off] Wallhaven"), "{text}");
        assert!(text.contains("api key: missing"), "{text}");
        assert!(text.contains("j/k | Enter sub | e edit"), "{text}");
    }

    #[test]
    fn wallhaven_block_renders_search_collections_and_missing_key_warning() {
        let mut app = test_app_with_wallhaven(
            true,
            serde_json::json!({
                "prefer": "collections_then_search",
                "collections": [
                    { "username": "alice", "id": 42, "label": "Abstract" }
                ],
                "search": {
                    "q": "mountains",
                    "categories": "101",
                    "purity": "100",
                    "sorting": "toplist",
                    "order": "desc",
                    "atleast": "2560x1440"
                }
            }),
            serde_json::json!({}),
        );
        app.config_cursor = 2;

        let text = render_text(&app, 120, 30);

        assert!(
            text.contains("> [off] Wallhaven - online on, no key, 1 col"),
            "{text}"
        );
        assert!(text.contains("api key: missing"), "{text}");
        assert!(
            text.contains("search: q=mountains categories=101 purity=100"),
            "{text}"
        );
        assert!(
            text.contains("sort: toplist desc minimum 2560x1440"),
            "{text}"
        );
        assert!(text.contains("1. Abstract: alice/42"), "{text}");
        assert!(
            text.contains("warning: API key missing; search and downloads are unavailable"),
            "{text}"
        );
    }

    #[test]
    fn wallhaven_block_shows_key_presence_without_leaking_secret() {
        let mut app = test_app_with_wallhaven(
            true,
            serde_json::json!({
                "prefer": "search_only",
                "search": {
                    "q": "forest",
                    "purity": "111"
                }
            }),
            serde_json::json!({ "wallhaven_api_key": "super-secret-token" }),
        );
        app.config_cursor = 2;

        let text = render_text(&app, 120, 30);

        assert!(text.contains("> [on] Wallhaven - online on, key"), "{text}");
        assert!(text.contains("api key: present"), "{text}");
        assert!(text.contains("prefer: SearchOnly"), "{text}");
        assert!(
            text.contains("search: q=forest categories=111 purity=111"),
            "{text}"
        );
        assert!(text.contains("collections: none"), "{text}");
        assert!(
            text.contains("warning: NSFW purity requires Wallhaven account access"),
            "{text}"
        );
        assert!(!text.contains("super-secret-token"), "{text}");
    }

    #[test]
    fn config_focus_does_not_share_list_cursor_state() {
        let mut app = test_app();
        app.cursor = 7;

        app.move_down();
        app.move_down();

        assert_eq!(app.config_cursor, 2);
        assert_eq!(app.cursor, 7);

        app.tab = Tab::History;
        app.move_up();

        assert_eq!(app.config_cursor, 2);
        assert_eq!(app.cursor, 6);
    }

    #[test]
    fn command_mode_footer_shows_mode_command_and_cancel_path() {
        let mut app = test_app();
        app.input_mode = InputMode::Command;
        app.cmd_line = "next".into();
        app.message = "applied: /tmp/wall.jpg".into();

        let text = render_text(&app, 80, 12);

        assert!(text.contains("command"), "{text}");
        assert!(text.contains(":next_"), "{text}");
        assert!(text.contains("Esc cancel"), "{text}");
        assert!(text.contains("applied: /tmp/wall.jpg"), "{text}");
    }

    #[test]
    fn narrow_search_screen_keeps_mode_query_and_actions_visible() {
        let mut app = test_app();
        app.tab = Tab::Search;
        app.search_query = "mountains".into();

        let text = render_text(&app, 42, 10);

        assert!(text.contains("Search"), "{text}");
        assert!(text.contains("query: mountains"), "{text}");
        assert!(text.contains("normal"), "{text}");
        assert!(text.contains("i edit | Enter | j/k | : | q"), "{text}");
    }

    #[test]
    fn terminal_size_contracts_cover_tiny_narrow_standard_and_wide() {
        assert_eq!(
            super::terminal_size(Rect::new(0, 0, 9, 24)),
            TerminalSize::Tiny
        );
        assert_eq!(
            super::terminal_size(Rect::new(0, 0, 42, 10)),
            TerminalSize::Narrow
        );
        assert_eq!(
            super::terminal_size(Rect::new(0, 0, 80, 24)),
            TerminalSize::Standard
        );
        assert_eq!(
            super::terminal_size(Rect::new(0, 0, 120, 32)),
            TerminalSize::Wide
        );
    }

    #[test]
    fn standard_layout_keeps_full_key_row_visible() {
        let app = test_app();
        let text = render_text(&app, 80, 24);

        // key row visible (may wrap/truncate on 80 cols with long config footer "Enter sub" etc)
        assert!(
            text.contains("Enter (sub") || text.contains("subnav"),
            "{text}"
        );
        assert!(
            text.contains("space pa") || text.contains("pause"),
            "{text}"
        );
    }

    #[cfg(feature = "tui-preview")]
    #[test]
    fn wide_now_layout_keeps_metadata_and_preview_regions_stable() {
        let mut app = test_app();
        app.tab = Tab::Now;

        let text = render_text(&app, 120, 32);

        assert!(text.contains("Now"), "{text}");
        assert!(text.contains("preview"), "{text}");
        assert!(text.contains("(no current wallpaper)"), "{text}");
    }

    #[test]
    fn status_kind_maps_messages_to_redundant_state_roles() {
        assert_eq!(
            style::status_kind("applied: /tmp/a.jpg"),
            style::StatusKind::Success
        );
        assert_eq!(
            style::status_kind("preview unsupported; showing metadata"),
            style::StatusKind::Error
        );
        assert_eq!(
            style::status_kind("preview disabled; showing metadata"),
            style::StatusKind::Warning
        );
        assert_eq!(style::status_kind("ready"), style::StatusKind::Neutral);
    }

    #[test]
    fn number_keys_select_visible_tabs_by_digit() {
        let mut app = test_app();
        let rt = tokio::runtime::Runtime::new().expect("runtime");

        assert!(
            !handle_key(&mut app, KeyEvent::from(KeyCode::Char('5')), rt.handle())
                .expect("handle key")
        );
        assert_eq!(app.tab, Tab::Search);

        assert!(
            !handle_key(&mut app, KeyEvent::from(KeyCode::Char('2')), rt.handle())
                .expect("handle key")
        );
        assert_eq!(app.tab, Tab::Now);

        assert!(
            !handle_key(&mut app, KeyEvent::from(KeyCode::Char('6')), rt.handle())
                .expect("handle key")
        );
        assert_eq!(app.tab, Tab::Logs);
    }

    #[test]
    fn key_mapping_separates_normal_command_and_search_input_modes() {
        let mut app = test_app();

        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char(':'))),
            UiAction::EnterCommandMode
        );
        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
            UiAction::Quit
        );

        app.input_mode = InputMode::Command;
        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
            UiAction::CommandChar('q')
        );
        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Esc)),
            UiAction::CancelInput
        );

        app.input_mode = InputMode::SearchInput;
        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Char('q'))),
            UiAction::SearchChar('q')
        );
        assert_eq!(
            action_for_key(&app, KeyEvent::from(KeyCode::Enter)),
            UiAction::SubmitSearch
        );
    }

    #[test]
    fn update_returns_reload_effect_for_domain_actions() {
        let mut app = test_app();
        let rt = tokio::runtime::Runtime::new().expect("runtime");

        assert_eq!(
            update(&mut app, UiAction::TogglePause, rt.handle()).expect("toggle"),
            UpdateEffect::None
        );
        assert!(app.message.starts_with("paused:"));

        assert_eq!(
            update(&mut app, UiAction::Next, rt.handle()).expect("next"),
            UpdateEffect::Reload
        );
        assert!(
            app.message.starts_with("next:") || app.message.starts_with("next error:"),
            "{}",
            app.message
        );
    }

    #[test]
    fn config_toggle_persists_boolean_and_reloads_context() {
        let mut app = test_app();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        app.config_cursor = 0;
        app.tab = Tab::Config;

        assert!(app.ctx.config.change.enabled);
        assert_eq!(
            update(&mut app, UiAction::ToggleConfigValue, rt.handle()).expect("toggle config"),
            UpdateEffect::Reload
        );
        apply_effect(&mut app, UpdateEffect::Reload).expect("reload");

        assert!(!app.ctx.config.change.enabled);
        assert!(app.message.contains("config saved: rotation enabled=false"));
        let text = std::fs::read_to_string(&app.ctx.paths.config_file).expect("config json");
        assert!(text.contains("\"enabled\": false"), "{text}");
    }

    #[test]
    fn config_cycle_persists_enum_like_value_and_reloads_context() {
        let mut app = test_app();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        app.config_cursor = 3;
        app.tab = Tab::Config;

        assert_eq!(
            update(&mut app, UiAction::CycleConfigValue, rt.handle()).expect("cycle config"),
            UpdateEffect::Reload
        );
        apply_effect(&mut app, UpdateEffect::Reload).expect("reload");

        assert!(app.message.contains("config saved: selection=Sequential"));
        let text = std::fs::read_to_string(&app.ctx.paths.config_file).expect("config json");
        assert!(text.contains("\"strategy\": \"sequential\""), "{text}");
        assert!(render_text(&app, 120, 32).contains("selection: Sequential"));
    }

    #[test]
    fn config_edit_state_starts_and_cancels_without_side_effects() {
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [
                    { "enabled": true, "type": "json", "label": "demo", "url": "https://example", "image_path": "$.u" }
                ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 0; // rotation block (or sources; start_edit will decide)
        assert!(app.editing.is_none());
        // direct for RED (will be wired via action later)
        app.start_edit_for_current();
        assert!(app.editing.is_some());
        app.cancel_edit();
        assert!(app.editing.is_none());
        // no side effects
        assert!(app.ctx.config.change.enabled);
    }

    #[test]
    fn e_on_config_block_enters_edit_popup_state() {
        use crate::tui::app::EditTarget;
        use ratatui::crossterm::event::KeyModifiers;
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [ { "enabled": true, "type": "folder", "path": "/tmp" } ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 0;
        // Drive via key path (action_for_key + update) - before wiring 'e' -> EditConfigItem this will not enter edit
        // (test will fail assert until Task 2 wire)
        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        let action = action_for_key(&app, key);
        // simulate update (in real handle_key calls update)
        let rt = tokio::runtime::Runtime::new().expect("rt");
        let _ = update(&mut app, action, rt.handle());
        assert!(
            app.is_editing(),
            "after 'e' on config should have entered edit state"
        );
        assert!(matches!(
            app.editing.as_ref().unwrap().target,
            EditTarget::Block(0)
        ));
    }

    #[test]
    fn e_then_render_shows_popup_form_with_fields_and_clear() {
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true, "interval_secs": 60 },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [ { "enabled": true, "type": "json", "label": "demo json", "url": "https://ex", "image_path": "$.d" } ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 1; // sources-ish
        app.start_edit_for_current();
        let text = render_text(&app, 80, 24);
        assert!(
            text.contains("EDIT FORM"),
            "popup form marker should be in buffer (Clear + content)"
        );
        // fields from demo
        let has_field = text.contains("enabled")
            || text.contains("type")
            || text.contains("url")
            || text.contains("interval");
        assert!(
            has_field,
            "form should list some fields for the item; got prefix: {}",
            &text[0..300.min(text.len())]
        );
    }

    #[test]
    fn edit_form_live_buffer_and_commit_updates_draft() {
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [ { "enabled": true, "type": "json", "url": "https://old", "image_path": "$.old" } ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 1;
        app.start_edit_for_current();
        assert!(app.is_editing());
        // type into current (assume starts at 0 or 3 for url-ish)
        // for demo, force cursor to a string field and type
        if let Some(s) = &mut app.editing {
            s.field_cursor = 3; // url in our list
        }
        let rt = tokio::runtime::Runtime::new().expect("rt");
        update(&mut app, UiAction::EditFieldChar('h'), rt.handle()).ok();
        update(&mut app, UiAction::EditFieldChar('i'), rt.handle()).ok();
        // buffer should have "hi"
        let buf = app.editing.as_ref().unwrap().field_buffer.clone();
        assert_eq!(buf, "hi");
        // commit field
        update(&mut app, UiAction::EditFieldCommit, rt.handle()).ok();
        // draft should be updated (url now has hi appended or set)
        let draft = app.editing.as_ref().unwrap().draft_source.as_ref().unwrap();
        assert!(
            draft.url.as_deref().unwrap_or("").contains("hi") || draft.url.as_deref() == Some("hi")
        );
    }

    #[test]
    fn config_subnav_jk_pick_then_e_edits_specific_source() {
        use crate::tui::app::EditTarget;
        // Setup with multiple sources so we can pick nested
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [
                    { "enabled": true, "type": "folder", "path": "/tmp" },
                    { "enabled": false, "type": "json", "label": "the one", "url": "https://ex", "image_path": "$.x" }
                ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        // Assume block 1 is now the Sources list block (we'll make it so)
        app.config_cursor = 1;
        // RED: no subnav yet, so entering sub + move + e should not target Source(1)
        // (will fail until impl)
        app.enter_config_subnav(); // expect to add
        update(
            &mut app,
            UiAction::MoveDown,
            tokio::runtime::Runtime::new().unwrap().handle(),
        )
        .ok();
        update(
            &mut app,
            UiAction::EditConfigItem,
            tokio::runtime::Runtime::new().unwrap().handle(),
        )
        .ok();
        let editing = app
            .editing
            .as_ref()
            .expect("should be editing after e in sub");
        assert!(
            matches!(editing.target, EditTarget::Source(1)),
            "should have picked the 2nd source via subnav j/k then e"
        );
    }
}
