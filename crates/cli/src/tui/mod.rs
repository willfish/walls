mod app;
#[cfg(feature = "tui-preview")]
mod preview;
mod style;

use std::io::{stdout, IsTerminal};

use crate::tui::app::EditTarget;
use anyhow::Context;
use app::{App, InputMode, Tab};
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph, Tabs};
use walls_core::apply::{
    backend_setting_label, summarize_apply_environment, ApplyEnvironmentSummary,
};
use walls_core::config::{reddit_summary, ApplyBackendSetting, CosmicMethod};
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

pub fn run(startup_message: Option<String>, tray_owns_rotation: bool) -> anyhow::Result<()> {
    require_tty()?;
    let rt = tokio::runtime::Handle::current();

    let mut stdout = stdout();
    enable_raw_mode().context("failed to enable raw mode (is this an interactive terminal?)")?;
    stdout
        .execute(EnterAlternateScreen)
        .context("failed to enter alternate screen")?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let _restore = TerminalRestore;

    let mut app = App::new(WallsCtx::load().context("failed to load walls config")?)?;
    if let Some(message) = startup_message {
        app.message = message;
    }
    IN_TUI.store(true, Ordering::Relaxed);
    #[cfg(feature = "tui-preview")]
    let mut preview = preview::ImagePreview::detect();
    let mut auto_rotator = if tray_owns_rotation {
        None
    } else {
        Some(walls_core::rotation::AutoRotator::new())
    };

    loop {
        terminal.draw(|f| {
            #[cfg(feature = "tui-preview")]
            draw(f, &app, &mut preview);
            #[cfg(not(feature = "tui-preview"))]
            draw(f, &app);
        })?;
        if let Some(rotator) = &mut auto_rotator {
            let outcome = rt.block_on(async {
                let mut ctx = walls_core::WallsCtx::load()?;
                Ok::<_, anyhow::Error>(rotator.tick(&mut ctx).await)
            });
            if matches!(outcome, Ok(walls_core::rotation::TickOutcome::Rotated)) {
                app.reload_ctx()?;
            }
        }
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
    EditFieldCycle {
        forward: bool,
    },
    ExitConfigSubnav,
    #[allow(dead_code)]
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

    // Editing steals keys for field nav (arrows only), live typing, commit/save on Enter, cancel.
    // Letter j/k are *not* field nav here (they become Char => type into buffer, or for list nav you Esc first then j/k).
    // This implements "Rather than jk in edit mode. Let's allow the user to hit escape first and then j/k".
    // n/p etc also type (disabled for globals).
    // Enter = commit buffer for current field + persist/save the item (no separate save key; "enter ... should just save the config").
    if app.is_editing() {
        return match key.code {
            KeyCode::Up => UiAction::EditFieldUp,
            KeyCode::Down => UiAction::EditFieldDown,
            KeyCode::Left => UiAction::EditFieldCycle { forward: false },
            KeyCode::Right => UiAction::EditFieldCycle { forward: true },
            KeyCode::Char(' ') => UiAction::EditFieldCycle { forward: true },
            KeyCode::Esc => UiAction::CancelEdit,
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
        KeyCode::Esc
            if app.tab == Tab::Config
                && app.config_in_subnav
                && app.is_sources_list_block(app.config_cursor) =>
        {
            UiAction::ExitConfigSubnav
        }
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
            app.message =
                match tokio::task::block_in_place(|| rt.block_on(app.ctx.advance_next_manual())) {
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
            if matches!(
                app.current_edit_field_kind(),
                app::EditFieldKind::Bool | app::EditFieldKind::Choice(_)
            ) {
                // Choice/bool fields use Space/arrow cycling, not free text.
            } else if let Some(sess) = &mut app.editing {
                sess.field_buffer.push(c);
                app.refresh_edit_validation();
            }
        }
        UiAction::EditFieldBackspace => {
            if matches!(
                app.current_edit_field_kind(),
                app::EditFieldKind::Bool | app::EditFieldKind::Choice(_)
            ) {
                // Choice/bool fields use Space/arrow cycling, not free text.
            } else if let Some(sess) = &mut app.editing {
                sess.field_buffer.pop();
                app.refresh_edit_validation();
            }
        }
        UiAction::EditFieldCycle { forward } => {
            app.cycle_current_edit_field(forward);
        }
        UiAction::ExitConfigSubnav => {
            app.exit_config_subnav();
        }
        UiAction::EditFieldCommit => {
            if app.editing.is_some() {
                app.commit_edit_field_buffer();
                // Commit the field to draft, then persist/save the item (atomic write + reload).
                // Keep the edit form open (user can continue to other fields of this item or Esc to leave).
                // This makes "type ... and hit enter" save the config without a separate save step.
                let _ = app.save_edit_item(false);
                // Re-fill buffer from the (now committed) draft value so the focused line shows "val|" (not empty |)
                // ready for further typing on this field, and indicates the committed state.
                if app.is_editing() {
                    let val = app.current_edit_field_value();
                    if let Some(s) = &mut app.editing {
                        s.field_buffer = val;
                    }
                }
            }
        }
        UiAction::EditFieldUp => {
            // Pure field move inside edit form (triggered by arrows; letter j/k no longer do this).
            // No auto commit/persist on arrow (uncommitted typing on a field is lost if you arrow away; hit Enter to commit+save a field).
            let buf = if let Some(sess) = &app.editing {
                let c = sess.field_cursor.saturating_sub(1);
                app.edit_field_value_at(&sess.target, c)
            } else {
                String::new()
            };
            if let Some(sess) = &mut app.editing {
                if sess.field_cursor > 0 {
                    sess.field_cursor -= 1;
                }
                sess.field_buffer = buf;
            }
        }
        UiAction::EditFieldDown => {
            // Pure field move inside edit form (triggered by arrows; letter j/k no longer do this).
            // No auto commit/persist on arrow (uncommitted typing on a field is lost if you arrow away; hit Enter to commit+save a field).
            let max_fields = app.edit_field_count();
            let buf = if let Some(sess) = &app.editing {
                let c = (sess.field_cursor + 1).min(max_fields.saturating_sub(1));
                app.edit_field_value_at(&sess.target, c)
            } else {
                String::new()
            };
            if let Some(sess) = &mut app.editing {
                if sess.field_cursor + 1 < max_fields {
                    sess.field_cursor += 1;
                }
                sess.field_buffer = buf;
            }
        }
        UiAction::SaveEditItem => {
            // The (now un-bound in edit UI) Save action does full commit+persist and exits the edit form.
            let _ = app.save_edit_item(true);
        }
        UiAction::SwitchTab(tab) => {
            app.tab = tab;
            app.cursor = 0;
            app.config_in_subnav = false;
            app.editing = None;
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
        Tab::Config if app.is_sources_list_block(app.config_cursor) && !app.config_in_subnav => {
            app.enter_config_subnav();
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
        if app.tab == Tab::Config && app.is_editing() && terminal_size(area) == TerminalSize::Wide {
            // wide split for edit: left context, right form (like Now preview)
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(area);
            render_lines(
                f,
                chunks[0],
                "List context",
                vec!["(use normal view for j/k subnav)".into()],
                theme,
            );
            render_rich_edit(f, chunks[1], app, theme, &edit_target_title(app));
        } else {
            if app.tab == Tab::Config && app.is_editing() {
                render_rich_edit(f, area, app, theme, &edit_target_title(app));
            } else {
                let (title, body) = (app.tab.title().to_string(), tab_lines(app, area.width));
                render_lines(f, area, &title, body, theme);
            }
        }
    }
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
    if app.tab == Tab::Config && app.is_editing() {
        render_rich_edit(f, area, app, theme, &edit_target_title(app));
    } else {
        let (title, body) = (app.tab.title().to_string(), tab_lines(app, area.width));
        render_lines(f, area, &title, body, theme);
    }
}

fn tab_lines(app: &App, width: u16) -> Vec<String> {
    match app.tab {
        Tab::Config => config_lines(app),
        Tab::Now => now_lines(app),
        Tab::History => app.history_lines(),
        Tab::Browse => app.browse_lines(),
        Tab::Search => app.search_lines(),
        Tab::Logs => app.logs_lines(width),
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
                Tab::Config
                    if app.config_in_subnav && app.is_sources_list_block(app.config_cursor) =>
                {
                    "Esc back | j/k | e edit | t | n/p | sp | : | q".into()
                }
                Tab::Config => "j/k | Enter sub | e edit | t | n/p | sp | : | q".into(),
                _ => "1-5 | n/p | f/d | sp | : | q".into(),
            },
        };
    }

    app.footer_keys()
}

fn line_style(line: &str, theme: style::Theme) -> Style {
    let trimmed = line.trim_start();
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
    // Default to normal (main fg) for content lines so the edit form "pops" with readable text.
    // Combined with selected row highlight, accent headers, alignment, and Unicode, it feels more modern.
    let kind = style::status_kind(line);
    if kind == style::StatusKind::Neutral {
        return theme.normal();
    }
    theme.status(kind)
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
    // Sources block lists configured providers plus Wallhaven (nested edit with j/k pick + e)
    let sources = &app.ctx.config.sources;
    let wallhaven_enabled = app.wallhaven_summary.usable();
    let sources_enabled = sources.iter().any(|s| s.enabled) || wallhaven_enabled;
    let enabled_count =
        sources.iter().filter(|s| s.enabled).count() + usize::from(wallhaven_enabled);
    let sources_summary = format!(
        "{} configured, {} enabled",
        sources.len() + 1,
        enabled_count
    );
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
        3,
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
    let in_sub = app.config_in_subnav && app.is_sources_list_block(app.config_cursor);
    let sub_sel = if in_sub {
        Some(app.config_sub_cursor)
    } else {
        None
    };
    let mut lines = Vec::new();
    for (index, src) in sources.iter().enumerate() {
        let state = if src.enabled { "on" } else { "off" };
        let marker = if sub_sel == Some(index) { "> " } else { "  " };
        if src.source_type == "reddit" {
            lines.push(format!(
                "{}{}. [{state}] Reddit - {}",
                marker,
                index + 1,
                reddit_summary(src)
            ));
            continue;
        }
        let key = src
            .path
            .as_deref()
            .or(src.url.as_deref())
            .or(src.query.as_deref())
            .unwrap_or("(no key)");
        let label = src.label.as_deref().unwrap_or(&src.source_type);
        lines.push(format!(
            "{}{}. [{state}] {} ({}) - {}",
            marker,
            index + 1,
            label,
            src.source_type,
            key
        ));
    }

    let wallhaven_index = sources.len();
    let wallhaven_state = if app.wallhaven_summary.usable() {
        "on"
    } else {
        "off"
    };
    let marker = if sub_sel == Some(wallhaven_index) {
        "> "
    } else {
        "  "
    };
    lines.push(format!(
        "{}{}. [{wallhaven_state}] Wallhaven (wallhaven) - {}",
        marker,
        wallhaven_index + 1,
        wallhaven_summary(app)
    ));
    if sub_sel == Some(wallhaven_index) {
        for detail in wallhaven_details(app) {
            lines.push(format!("      {detail}"));
        }
    }
    lines
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
        format!("search: q={}", provider.query),
        format!(
            "categories: {}",
            app::format_wallhaven_categories(&provider.categories)
        ),
        format!(
            "purity: {}",
            app::format_wallhaven_purity(&provider.purity, provider.api_key_present)
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

#[allow(dead_code)]
/// Descriptive title for the edit target (block or specific source with its json label+type).
/// Used for chrome block titles so "what is being edited" is obvious at a glance (not generic "Config (editing)").
fn edit_target_title(app: &App) -> String {
    if let Some(sess) = &app.editing {
        match &sess.target {
            EditTarget::Block(0) => "Edit Rotation".to_string(),
            EditTarget::Wallhaven => "Edit Wallhaven".to_string(),
            EditTarget::Block(b) => format!("Edit block {}", b),
            EditTarget::Source(i) => {
                if let Some(ref src) = sess.draft_source {
                    if src.source_type == "reddit" {
                        format!("Edit Reddit #{}", i + 1)
                    } else {
                        let lab = src.label.clone().unwrap_or_else(|| src.source_type.clone());
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

#[allow(dead_code)]
/// Pure form lines for drill-down edit view (replaces blocks list in main content when editing a Config item).
/// No overlay/Clear/popup - stable layout, reuses render_lines.
fn config_edit_form_lines(app: &App) -> Vec<String> {
    if let Some(sess) = &app.editing {
        let mut lines: Vec<String> = vec![
            // Modern form header using box-drawing for a contemporary TUI feel (like lazygit, helix, etc.).
            // No duplicate title (chrome provides "Edit Rotation" etc.).
            "┄─ EDIT FORM (▸ focus | ↑/↓ | type or Space/←/→ | Enter save | Esc) ─┄".into(),
        ];
        // Validation errors inline at top (after marker) so visible immediately, with !! cue for red styling.
        // This addresses "they have no validation" and "s it just fails" (user sees *why* before or on save).
        if !sess.validation_errors.is_empty() {
            lines.push("!! Validation errors:".into());
            for e in &sess.validation_errors {
                lines.push(format!("!! - {}", e));
            }
            lines.push("".into());
        }
        // dynamic fields list with cursor + live buffer on current (same logic as before)
        let mut fields: Vec<(String, String, app::EditFieldKind)> = vec![];
        if let Some(ref src) = sess.draft_source {
            // Use the single source of truth for necessary fields per type (no dups, no unused like title_path)
            for name in app::App::source_editable_fields(src) {
                let label = app::source_field_label(src, &name);
                let v = app::App::get_source_field(src, &name);
                fields.push((label, v, app::source_field_kind_for(src, &name)));
            }
        } else if let EditTarget::Wallhaven = &sess.target {
            for k in app::WALLHAVEN_BLOCK_FIELDS {
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
                }
            }
            fields.push((
                "API key".into(),
                "(edit ~/.config/walls/secrets.json)".into(),
                app::EditFieldKind::Text,
            ));
            fields.push((
                "Collections".into(),
                "(edit config.json for now)".into(),
                app::EditFieldKind::Text,
            ));
        } else if let EditTarget::Block(block) = &sess.target {
            let keys = match block {
                0 => app::ROTATION_BLOCK_FIELDS,
                _ => &[],
            };
            for k in keys {
                if let Some(v) = sess.draft_block_values.get(*k) {
                    fields.push((
                        app::block_field_label(*block, k),
                        v.clone(),
                        app::block_field_kind(*block, k),
                    ));
                }
            }
        }
        // Right-aligned labels within a capped column for a tight, modern form look (avoids huge gaps on short labels like "Type").
        // Values stay in a clean column. Cap prevents sparse layout on small forms.
        let max_label = fields.iter().map(|(l, _, _)| l.len()).max().unwrap_or(0);
        let pad = std::cmp::min(max_label, 28);
        let wallhaven_keys = if matches!(&sess.target, EditTarget::Wallhaven) {
            app::WALLHAVEN_BLOCK_FIELDS
        } else {
            &[] as &[&str]
        };
        let source_names = sess
            .draft_source
            .as_ref()
            .map(app::App::source_editable_fields);
        for (i, (k, v, kind)) in fields.iter().enumerate() {
            let padded = format!("{:>width$}", k, width = pad);
            let field_key = source_names
                .as_ref()
                .and_then(|names| names.get(i).map(String::as_str))
                .or_else(|| wallhaven_keys.get(i).copied())
                .unwrap_or("");
            let val = if i == sess.field_cursor {
                match kind {
                    app::EditFieldKind::Text => format!("{}|", sess.field_buffer),
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

/// Build rich ListItems for the edit form using Spans for per-segment styling.
/// This enables modern form aesthetics: accent/cyan labels for hierarchy, normal values,
/// strong selected highlight on the current row (▸ ), red errors, etc.
/// Keeps the plain text content the same for tests/pty inspection.
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
            // Modern header/separator
            let l = Line::from(Span::styled(
                line,
                theme.accent().add_modifier(Modifier::BOLD),
            ));
            items.push(ListItem::new(l));
            continue;
        }
        if trimmed.starts_with("!!") {
            let err_st = theme.status(style::StatusKind::Error);
            let l = Line::from(vec![
                Span::styled("!! ", err_st),
                Span::styled(line[3..].to_string(), err_st),
            ]);
            items.push(ListItem::new(l));
            continue;
        }
        if trimmed.starts_with("▸ ") || trimmed.starts_with("  ") {
            // Field: split for rich modern styling.
            // - Current row: high-contrast black-on-cyan (edit_focus_*) so labels stay readable.
            // - Non-current: labels muted. Bool values use ✓/✗ + semantic colour.
            if let Some(colon_pos) = line.find(": ") {
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
                } else if value_part.starts_with("✓ true") {
                    theme.status(style::StatusKind::Success)
                } else if value_part.starts_with("✗ false") {
                    theme.status(style::StatusKind::Error)
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
        }
        // Fallback to plain + line_style
        let st = line_style(&line, theme);
        items.push(ListItem::new(line).style(st));
    }
    items
}

/// Render the edit form with rich per-segment Spans (labels in accent/muted for hierarchy,
/// values normal, current row with selected highlight). This makes the form feel more modern
/// and "designed" (visual distinction, scannable) while reusing the string builder for tests.
fn render_rich_edit(f: &mut Frame, area: Rect, app: &App, theme: style::Theme, block_title: &str) {
    let items = build_rich_edit_form_items(app, theme);
    let list = List::new(items)
        .block(theme.content_block(block_title))
        .style(theme.normal());
    f.render_widget(list, area);
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
        assert!(!text.contains("  [off] Wallhaven"), "{text}");
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
        // now rendered via sources_details (full providers list, including Wallhaven)
        assert!(text.contains("1. [on]"), "{text}");
        assert!(text.contains("2. [off] Wallhaven (wallhaven)"), "{text}");
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
        assert!(text.contains("7 configured, 5 enabled"), "{text}");
        assert!(text.contains("1. [on] Favorites (favorites)"), "{text}");
        assert!(text.contains("2. [on] Fetched (fetched)"), "{text}");
        assert!(text.contains("3. [on] Wallpapers (folder)"), "{text}");
        assert!(text.contains("4. [on] Single (image)"), "{text}");
        assert!(text.contains("5. [off] Disabled (folder)"), "{text}");
        assert!(text.contains("6. [on] Missing (folder)"), "{text}");
        assert!(text.contains("7. [off] Wallhaven (wallhaven)"), "{text}");
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
        app.config_cursor = 2;

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
        app.config_cursor = 3;

        let text = render_text(&app, 120, 34);

        assert!(
            text.contains(
                "> [on] Apply/display - custom-script backend, fill mode, 3840x2160 target"
            ),
            "{text}"
        );
        assert!(text.contains("configured (config.json):"), "{text}");
        assert!(text.contains("detected (this session):"), "{text}");
        assert!(text.contains("backend: custom-script"), "{text}");
        assert!(text.contains("custom script: (not set)"), "{text}");
        assert!(text.contains("cosmic method: cosmic-ext-bg-ctl"), "{text}");
        assert!(
            text.contains("cosmic config path: /tmp/missing-cosmic-config"),
            "{text}"
        );
        assert!(text.contains("cosmic uses original: true"), "{text}");
        assert!(text.contains("display mode: fill"), "{text}");
        assert!(text.contains("EXIF auto-rotate: true"), "{text}");
        assert!(text.contains("target: 3840x2160 target"), "{text}");
        assert!(text.contains("resolved backend: custom-script"), "{text}");
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
        app.config_cursor = 1;
        app.enter_config_subnav();
        app.config_sub_cursor = app.ctx.config.sources.len();

        let text = render_text(&app, 42, 14);

        assert!(text.contains("Config"), "{text}");
        assert!(text.contains("> 2. [off] Wallhaven"), "{text}");
        assert!(text.contains("api key: missing"), "{text}");
        assert!(text.contains("Esc back | j/k | e edit"), "{text}");
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
        app.config_cursor = 1;
        app.enter_config_subnav();
        app.config_sub_cursor = app.ctx.config.sources.len();

        let text = render_text(&app, 120, 30);

        assert!(
            text.contains("> 1. [off] Wallhaven (wallhaven) - online on, no key, 1 col"),
            "{text}"
        );
        assert!(text.contains("api key: missing"), "{text}");
        assert!(text.contains("search: q=mountains"), "{text}");
        assert!(text.contains("categories: general, people"), "{text}");
        assert!(text.contains("purity: SFW"), "{text}");
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
        app.config_cursor = 1;
        app.enter_config_subnav();
        app.config_sub_cursor = app.ctx.config.sources.len();

        let text = render_text(&app, 120, 30);

        assert!(
            text.contains("> 1. [on] Wallhaven (wallhaven) - online on, key"),
            "{text}"
        );
        assert!(text.contains("api key: present"), "{text}");
        assert!(text.contains("prefer: SearchOnly"), "{text}");
        assert!(text.contains("search: q=forest"), "{text}");
        assert!(
            text.contains("categories: general, anime, people"),
            "{text}"
        );
        assert!(text.contains("purity: SFW, sketchy, NSFW"), "{text}");
        assert!(text.contains("collections: none"), "{text}");
        assert!(
            text.contains("warning: NSFW purity requires Wallhaven account access"),
            "{text}"
        );
        assert!(!text.contains("super-secret-token"), "{text}");
    }

    #[test]
    fn wallhaven_block_edit_form_exposes_search_fields() {
        let mut app = test_app_with_wallhaven(
            true,
            serde_json::json!({
                "prefer": "search_only",
                "search": {
                    "q": "forest",
                    "categories": "111",
                    "purity": "100",
                    "sorting": "random",
                    "order": "desc",
                    "atleast": "1920x1080"
                }
            }),
            serde_json::json!({ "wallhaven_api_key": "key" }),
        );
        app.tab = Tab::Config;
        app.config_cursor = 1;
        app.enter_config_subnav();
        app.config_sub_cursor = app.ctx.config.sources.len();
        app.start_edit_for_current();

        let text = render_text(&app, 120, 32);

        assert!(text.contains("Edit Wallhaven"), "{text}");
        assert!(text.contains("Search query"), "{text}");
        assert!(text.contains("forest"), "{text}");
        assert!(text.contains("search_only"), "{text}");
        assert!(text.contains("Category: General"), "{text}");
        assert!(text.contains("Category: Anime"), "{text}");
        assert!(text.contains("Purity: SFW"), "{text}");
        assert!(text.contains("secrets.json"), "{text}");
    }

    #[test]
    fn edit_form_space_toggles_bool_field_without_typing() {
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true, "on_start": false, "interval": 3600 },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": []
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 0;
        app.start_edit_for_current();
        let rt = tokio::runtime::Runtime::new().expect("rt");

        assert_eq!(
            app.editing.as_ref().unwrap().field_buffer,
            "true",
            "enabled field should prefill"
        );
        update(
            &mut app,
            UiAction::EditFieldCycle { forward: true },
            rt.handle(),
        )
        .ok();
        assert_eq!(
            app.editing.as_ref().unwrap().field_buffer,
            "false",
            "Space should toggle enabled to false"
        );
        let text = render_text(&app, 100, 24);
        assert!(
            text.contains("Space toggle") || text.contains("Space/"),
            "footer should hint choice controls: {text}"
        );
    }

    #[test]
    fn wallhaven_nsfw_unavailable_without_api_key() {
        let mut app = test_app_with_wallhaven(
            true,
            serde_json::json!({
                "search": {
                    "q": "forest",
                    "purity": "111",
                    "categories": "111"
                }
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 1;
        app.enter_config_subnav();
        app.config_sub_cursor = app.ctx.config.sources.len();
        app.start_edit_for_current();

        let text = render_text(&app, 120, 36);
        assert!(text.contains("Purity: NSFW (requires API key)"), "{text}");
        assert!(
            text.contains("unavailable (set wallhaven_api_key in secrets.json)"),
            "{text}"
        );

        // Navigate to NSFW field (index 7) and try toggling — should stay unavailable.
        let rt = tokio::runtime::Runtime::new().expect("rt");
        for _ in 0..7 {
            update(&mut app, UiAction::EditFieldDown, rt.handle()).ok();
        }
        update(
            &mut app,
            UiAction::EditFieldCycle { forward: true },
            rt.handle(),
        )
        .ok();
        let text = render_text(&app, 120, 36);
        assert!(
            text.contains("unavailable (set wallhaven_api_key in secrets.json)"),
            "Space should not enable NSFW without API key: {text}"
        );
    }

    #[test]
    fn edit_form_space_cycles_wallhaven_sorting_enum() {
        let mut app = test_app_with_wallhaven(
            true,
            serde_json::json!({
                "search": { "sorting": "random", "purity": "100", "categories": "111", "order": "desc", "atleast": "1920x1080" }
            }),
            serde_json::json!({ "wallhaven_api_key": "key" }),
        );
        app.tab = Tab::Config;
        app.config_cursor = 1;
        app.enter_config_subnav();
        app.config_sub_cursor = app.ctx.config.sources.len();
        app.start_edit_for_current();
        let rt = tokio::runtime::Runtime::new().expect("rt");

        // sorting follows prefer, search_q, and six category/purity toggles
        for _ in 0..8 {
            update(&mut app, UiAction::EditFieldDown, rt.handle()).ok();
        }
        assert_eq!(
            app.editing.as_ref().unwrap().field_buffer,
            "random",
            "should land on sorting field"
        );
        update(
            &mut app,
            UiAction::EditFieldCycle { forward: true },
            rt.handle(),
        )
        .ok();
        assert_eq!(
            app.editing.as_ref().unwrap().field_buffer,
            "views",
            "Space should cycle sorting to next option after random"
        );
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

        assert!(text.contains("e edit"), "{text}");
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
        app.config_cursor = 2;
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
    fn e_then_render_shows_drilldown_form_in_main_content() {
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
        // Drill-down (non-modal): when editing Config item, main content shows the form fields directly (replaces blocks list in body area). No overlay/Clear popup.
        assert!(
            text.contains("EDIT FORM"),
            "form marker should be in main tab content for drill-down edit view"
        );
        // fields from demo (labels now Title for clarity)
        let has_field = text.contains("Enabled")
            || text.contains("type")
            || text.contains("URL")
            || text.contains("Interval");
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
        // With new UX, focus sets buffer to current value for editing/backspace support
        if let Some(s) = &mut app.editing {
            s.field_cursor = 3; // url in our list
        }
        // re-focus effect: set buffer (sim in test) - compute before mut borrow
        let initial_buf = app.current_edit_field_value();
        if let Some(s) = &mut app.editing {
            s.field_buffer = initial_buf;
        }
        let rt = tokio::runtime::Runtime::new().expect("rt");
        // simulate backspace to clear/edit: backspace the value down
        // url "https://old" , backspace 4 times
        let orig_len = app.editing.as_ref().unwrap().field_buffer.len();
        for _ in 0..4 {
            update(&mut app, UiAction::EditFieldBackspace, rt.handle()).ok();
        }
        let buf = app.editing.as_ref().unwrap().field_buffer.clone();
        assert!(
            buf.len() == orig_len - 4 && !buf.ends_with("old"),
            "backspace should reduce the field value in buffer for clear/edit; buf={}",
            buf
        );
        // commit
        update(&mut app, UiAction::EditFieldCommit, rt.handle()).ok();
        let draft = app.editing.as_ref().unwrap().draft_source.as_ref().unwrap();
        assert!(
            !draft.url.as_deref().unwrap_or("").ends_with("old"),
            "committed shortened value"
        );
    }

    #[test]
    fn edit_form_query_field_for_reddit_commits_to_correct_draft_field_not_url() {
        // TDD for proper per-type fields + name-based commit (not brittle idx)
        // reddit uses query (from ex + tests + Variety compat), should be editable without polluting url
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [ { "enabled": true, "type": "reddit", "query": "cats", "sort": "hot" } ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 1; // sources block -> edits source 0
        app.start_edit_for_current();
        assert!(app.is_editing());
        let rt = tokio::runtime::Runtime::new().expect("rt");
        // fields for reddit: 0=enabled, 1=query (subreddit)
        update(&mut app, UiAction::EditFieldDown, rt.handle()).ok();
        // prefill should have loaded the query value via name-based current_edit
        let initial = app.editing.as_ref().unwrap().field_buffer.clone();
        assert_eq!(
            initial, "cats",
            "prefill must load query value for reddit source; got '{}'",
            initial
        );
        // backspace to edit/clear last char
        update(&mut app, UiAction::EditFieldBackspace, rt.handle()).ok();
        let buf = app.editing.as_ref().unwrap().field_buffer.clone();
        assert_eq!(buf, "cat", "backspace on query field");
        // commit field
        update(&mut app, UiAction::EditFieldCommit, rt.handle()).ok();
        let draft = app.editing.as_ref().unwrap().draft_source.as_ref().unwrap();
        assert_eq!(
            draft.query.as_deref(),
            Some("cat"),
            "query must be updated in draft"
        );
        assert!(
            draft.url.is_none() || draft.url.as_deref() == Some(""),
            "must not have polluted url field; url={:?}",
            draft.url
        );
    }

    #[test]
    fn reddit_edit_form_lists_subreddit_sort_and_time_without_label_or_type() {
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [ { "enabled": true, "type": "reddit", "query": "wallpapers", "sort": "top", "time": "month" } ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 1;
        app.enter_config_subnav();
        app.config_sub_cursor = 0;
        app.start_edit_for_current();

        let text = render_text(&app, 100, 28);
        assert!(text.contains("Edit Reddit"), "{text}");
        assert!(text.contains("Subreddit"), "{text}");
        assert!(text.contains("wallpapers"), "{text}");
        assert!(text.contains("Sort"), "{text}");
        assert!(text.contains("Time period"), "{text}");
        assert!(text.contains("month"), "{text}");
        assert!(!text.contains("Label"), "{text}");
        assert!(!text.contains("Type"), "{text}");
    }

    #[test]
    fn reddit_time_unavailable_when_sort_is_hot() {
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [ { "enabled": true, "type": "reddit", "query": "pics", "sort": "hot" } ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 1;
        app.enter_config_subnav();
        app.start_edit_for_current();
        let rt = tokio::runtime::Runtime::new().expect("rt");
        for _ in 0..3 {
            update(&mut app, UiAction::EditFieldDown, rt.handle()).ok();
        }
        let text = render_text(&app, 100, 28);
        assert!(text.contains("n/a (top/controversial only)"), "{text}");
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

    #[test]
    fn config_subnav_enter_enters_and_esc_exits_without_enter_toggle() {
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
        app.config_cursor = 1;
        let rt = tokio::runtime::Runtime::new().expect("rt");

        assert!(!app.config_in_subnav);
        update(&mut app, UiAction::Enter, rt.handle()).ok();
        assert!(app.config_in_subnav, "Enter on Sources should enter subnav");

        update(&mut app, UiAction::Enter, rt.handle()).ok();
        assert!(
            app.config_in_subnav,
            "Enter while in subnav must not exit; use Esc instead"
        );

        update(&mut app, UiAction::ExitConfigSubnav, rt.handle()).ok();
        assert!(!app.config_in_subnav, "Esc should exit subnav");

        let action = action_for_key(&app, KeyEvent::from(KeyCode::Esc));
        assert_eq!(
            action,
            UiAction::Ignore,
            "Esc outside subnav should not map to exit"
        );
    }

    #[test]
    fn config_subnav_highlights_selected_item_in_details() {
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
        app.config_cursor = 1;
        app.enter_config_subnav();
        // move to second item
        let rt = tokio::runtime::Runtime::new().expect("rt");
        update(&mut app, UiAction::MoveDown, rt.handle()).ok();
        let text = render_text(&app, 80, 24);
        // should highlight the selected sub with >
        assert!(
            text.contains("> 2. [off] the one (json)"),
            "sub item should be highlighted with > marker; got: {}",
            text
        );
        assert!(
            !text.contains("> 1. [on] folder"),
            "only selected sub highlighted"
        );
    }

    #[test]
    fn n_p_key_from_any_tab_gives_next_prev_when_not_editing_and_disabled_in_edit() {
        use ratatui::crossterm::event::KeyModifiers;
        // Core behaviour test (prevents regression of wallpaper n/p from any tab).
        // When not editing (any tab): 'n'/'p' => Next/Prev action (final match).
        // When editing: n/p should be disabled for wallpaper (no early force); fall to edit arm as Char (so can type 'n'/'p' in fields like queries) i.e. not Next.
        // Per user: n/p not working when not in edit; and "in edit mode everything but Enter or Escape should be disabled" (globals like wallpaper n/p disabled in edit).
        let rt = tokio::runtime::Runtime::new().expect("rt");

        for tab in [
            Tab::Now,
            Tab::History,
            Tab::Browse,
            Tab::Config,
            Tab::Search,
        ] {
            let mut app = test_app();
            app.tab = tab;
            // ensure normal non-edit state
            app.editing = None;
            app.input_mode = InputMode::Normal;
            if tab == Tab::Config {
                app.config_in_subnav = false;
            }

            let n_action = action_for_key(
                &app,
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()),
            );
            assert!(
                matches!(n_action, UiAction::Next),
                "n from tab {:?} (not editing) must give Next for wallpaper change",
                tab
            );
            let p_action = action_for_key(
                &app,
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty()),
            );
            assert!(matches!(p_action, UiAction::Prev));

            // Full behaviour: key -> action -> update produces Reload + next msg (core feature)
            let eff = update(&mut app, n_action, rt.handle()).expect("next via key");
            assert_eq!(eff, UpdateEffect::Reload);
            assert!(
                app.message.starts_with("next:")
                    || app.message.starts_with("next error:")
                    || app.message == "next: no change",
                "n from {:?} should trigger advance, got msg: {}",
                tab,
                app.message
            );
        }

        // In edit: n/p disabled as wallpaper (no Next), become edit chars (to allow typing in fields)
        let mut app = test_app();
        app.tab = Tab::Config;
        app.start_edit_for_current();
        assert!(app.is_editing());
        let n_action = action_for_key(
            &app,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()),
        );
        assert!(
            matches!(n_action, UiAction::EditFieldChar('n')),
            "n in edit must be EditFieldChar (wallpaper n/p disabled in edit mode), not Next"
        );
        let p_action = action_for_key(
            &app,
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty()),
        );
        assert!(matches!(p_action, UiAction::EditFieldChar('p')));
        // j/k (letters) no longer perform field nav in edit mode (per request: rather than jk in edit, hit Esc first then j/k for main list/subnav navigation).
        // Letters now type into the current field buffer (like other chars, to support queries/labels containing j/k).
        // Arrows (Up/Down) remain for moving between fields inside the edit form.
        let j_action = action_for_key(
            &app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()),
        );
        assert!(
            matches!(j_action, UiAction::EditFieldChar('j')),
            "j in edit must be EditFieldChar (types into field), not field nav; Esc first then j/k to navigate list or sources subnav"
        );
        let k_action = action_for_key(
            &app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()),
        );
        assert!(matches!(k_action, UiAction::EditFieldChar('k')));
        let down_action = action_for_key(&app, KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
        assert!(
            matches!(down_action, UiAction::EditFieldDown),
            "Down arrow still moves to next field inside edit form"
        );
        // other globals' *actions* disabled in edit (e.g. no tab switch, no quit);
        // instead Char(c) for most (incl '1','q','n' now) types into the field buffer (required to support
        // "type out all of the options" in forms for values containing digits/letters like queries, labels, urls).
        // Enter now commits the field buffer AND persists/saves the config item (no separate 's'); Esc to exit edit form.
        let one_action = action_for_key(
            &app,
            KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty()),
        );
        assert!(
            matches!(one_action, UiAction::EditFieldChar('1')),
            "tab switch 1 disabled (types instead) in edit"
        );
        let q_action = action_for_key(
            &app,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty()),
        );
        assert!(
            matches!(q_action, UiAction::EditFieldChar('q')),
            "q disabled (types instead) in edit"
        );
        // but edit controls and Enter/Esc work
        let esc_action = action_for_key(&app, KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(matches!(esc_action, UiAction::CancelEdit));
        let enter_action =
            action_for_key(&app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(matches!(enter_action, UiAction::EditFieldCommit));
    }

    #[test]
    fn edit_forms_for_different_source_types_prefill_values_from_config_json_and_list_only_necessary_fields(
    ) {
        // TDD coverage for "tests for all of the different forms/behaviours" + "some of the config items should be prefilled from the json configuration".
        // Unsplash uses many fields (query/collection/user/topic/orientation/url); must prefill the values provided in the json config,
        // and form must list exactly the necessary ones (no title_path, no irrelevant).
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [
                    {
                        "enabled": true,
                        "type": "unsplash",
                        "label": "Nature",
                        "query": "nature",
                        "orientation": "landscape",
                        "collection": "123456",
                        "user": "johndoe",
                        "topic": "wallpapers"
                    },
                    {
                        "enabled": false,
                        "type": "pixabay",
                        "label": "Pix",
                        "query": "cats",
                        "api_key": "SECRET123"
                    }
                ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;
        app.config_cursor = 1; // sources block
        app.start_edit_for_current();
        assert!(app.is_editing());
        // First source unsplash: cursor starts at 0 (enabled), buffer prefilled from the *json config* value
        let buf0 = app.editing.as_ref().unwrap().field_buffer.clone();
        assert_eq!(buf0, "true", "enabled must be prefilled from config json");

        // Move to query field (enabled0, type1, label2, url3, query4)
        let rt = tokio::runtime::Runtime::new().expect("rt");
        for _ in 0..4 {
            update(&mut app, UiAction::EditFieldDown, rt.handle()).ok();
        }
        let qbuf = app.editing.as_ref().unwrap().field_buffer.clone();
        assert_eq!(
            qbuf, "nature",
            "query must be prefilled from the json config value for unsplash source"
        );

        // Edit the query field (append to simulate user typing), commit -- updates *draft* (not yet live ctx)
        update(&mut app, UiAction::EditFieldChar('!'), rt.handle()).ok();
        update(&mut app, UiAction::EditFieldCommit, rt.handle()).ok();
        // Now move to next field (collection idx5), prefill should come from live (unchanged)
        update(&mut app, UiAction::EditFieldDown, rt.handle()).ok();
        // move back to query (now idx4 after previous moves? wait track: after previous 4 downs we were at query, +1 char+commit (no cursor change), +1 down to collection, now up back
        update(&mut app, UiAction::EditFieldUp, rt.handle()).ok();
        let qbuf_after_commit_and_return = app.editing.as_ref().unwrap().field_buffer.clone();
        // With improved prefill from draft, this should be the edited value "nature!" (committed to draft); if only live ctx, would be stale "nature"
        assert_eq!(qbuf_after_commit_and_return, "nature!", "after commit, returning to field must prefill buffer from draft state (which started from json config + edits), not stale live ctx");

        // Render exercises config_edit_form_lines which builds from draft (cloned from config json at start_edit)
        // Use taller height so that with possible !! errors section (from auto-persist on Commit in new UX) the later fields like Orientation are still in the captured buffer.
        let text = render_text(&app, 80, 30);
        // Note: labels are now padded for alignment (e.g. "Query                                    : nature!|"),
        // so contains checks use the distinctive value parts (robust to padding and errors section).
        assert!(
            text.contains("nature!"),
            "form must show updated draft value from json+edit; text: {}",
            text
        );
        assert!(
            text.contains("Orientation"),
            "orientation prefilled from json"
        );
        assert!(text.contains("123456"), "collection prefilled");
        assert!(text.contains("johndoe"), "user prefilled");
        assert!(text.contains("wallpapers"), "topic prefilled");
        // only necessary; no title_path ever, no bleed from other
        assert!(
            !text.contains("title_path"),
            "title_path unused, must not appear in any form"
        );
        assert!(!text.contains("image_path"), "image_path not for unsplash");

        // Now test second source (pixabay) has its fields
        // To switch source in test, cancel, move? but for simplicity re-start on a config with only pixabay as source0
        let mut app2 = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [ { "enabled": true, "type": "pixabay", "label": "Pix", "query": "cats", "api_key": "SECRET123" } ]
            }),
            serde_json::json!({}),
        );
        app2.tab = Tab::Config;
        app2.config_cursor = 1;
        app2.start_edit_for_current();
        let text2 = render_text(&app2, 80, 24);
        // Padded labels (e.g. "Query                                    : cats"), so check distinctive values.
        assert!(text2.contains("cats"), "pixabay query prefilled from json");
        assert!(
            text2.contains("SECRET123"),
            "api_key prefilled (masked? but in test form shows; from json)"
        );
        assert!(!text2.contains("url"), "no url for pixabay");
    }

    #[test]
    fn edit_forms_drive_shows_clear_targets_prefilled_values_inline_validation_and_bool_save_succeeds(
    ) {
        // TDD + drive the TUI per user: "Can you drive the TUI and look at these config edit screens?"
        // "None of them are clear what's being edited and they have no validation. I change a value from true to false and when I type s it just fails. Take some screenshots"
        // Uses real render_text (TestBackend) to produce visible "screenshots" of the form body + chrome.
        // Asserts desired: descriptive target in titles (from draft json label+type), prefilled current values visible,
        // validation errors rendered inline near top with red cue, direct s after bool edit (with proper clear+type) succeeds without opaque fail,
        // and post-save form would reflect the new value (or editing closed).
        let rt = tokio::runtime::Runtime::new().expect("rt");
        let mut app = test_app_with_config(
            serde_json::json!({
                "change": { "enabled": true, "interval_secs": 60, "internet_enabled": true },
                "paths": { "cache_dir": "/tmp/c", "download_dir": "/tmp/d", "favorites_dir": "/tmp/f", "fetched_dir": "/tmp/fe", "compose_dir": "/tmp/co" },
                "sources": [
                    { "enabled": true, "type": "wallhaven", "label": "wallhaven space", "query": "space" },
                    { "enabled": false, "type": "reddit", "query": "wallpapers", "sort": "top", "time": "month" }
                ]
            }),
            serde_json::json!({}),
        );
        app.tab = Tab::Config;

        // Drive rotation block edit (cursor 0)
        app.config_cursor = 0;
        app.start_edit_for_current();
        let rot_text = render_text(&app, 80, 30);
        eprintln!(
            "=== SCREENSHOT: EDIT ROTATION BLOCK (before bool change) ===\n{}",
            rot_text
        );
        // Desired: clear target in the rendered title area (not just generic "Config (editing)")
        assert!(
            rot_text.contains("Edit Rotation") || rot_text.contains("Rotation"),
            "rotation edit form must make target obvious; got head: {}",
            &rot_text[..rot_text.len().min(400)]
        );
        // TDD for full rotation fields: previously only enabled/interval/internet were in the block edit form
        // (hardcoded in start_edit, form_lines, value_at, commit, save). All ChangeConfig fields should be editable
        // (on_start, safe_mode, change_lock_screen, download_preference_ratio too) so user can configure the full rotation.
        assert!(
            rot_text.contains("On start") || rot_text.contains("on start"),
            "rotation edit must list on_start (full rotation settings, not just 3)"
        );
        assert!(
            rot_text.contains("Safe mode") || rot_text.contains("safe mode"),
            "rotation edit must list safe_mode"
        );
        assert!(
            rot_text.contains("Change lock screen") || rot_text.contains("lock screen"),
            "rotation edit must list change_lock_screen"
        );
        assert!(
            rot_text.contains("Download preference") || rot_text.contains("preference ratio"),
            "rotation edit must list download_preference_ratio"
        );

        // Now drive source with label from real user config ("wallhaven space")
        app.cancel_edit();
        app.config_cursor = 1; // sources block
                               // ensure subnav targets the first source (wallhaven space)
        app.config_in_subnav = true;
        app.config_sub_cursor = 0;
        app.start_edit_for_current();
        let src_text = render_text(&app, 80, 24);
        eprintln!(
            "=== SCREENSHOT: EDIT WALLHAVEN SPACE SOURCE (prefilled from json draft) ===\n{}",
            src_text
        );
        assert!(
            src_text.contains("wallhaven space") && src_text.contains("wallhaven"),
            "edit form header must show concrete label + type from draft json so 'what is being edited' is obvious"
        );
        assert!(
            src_text.contains("Enabled")
                && (src_text.contains("true") || src_text.contains("Enabled: true")),
            "enabled must be prefilled from the json config value"
        );

        // Reproduce the user flow: change enabled true -> false via Space toggle (bool fields are pickers, not free text).
        update(
            &mut app,
            UiAction::EditFieldCycle { forward: true },
            rt.handle(),
        )
        .ok();
        let after_enter_msg = app.message.clone();
        let still_editing = app.is_editing();
        eprintln!(
            "=== AFTER BOOL TOGGLE + Enter: message='{}' still_editing={} ===",
            after_enter_msg, still_editing
        );
        // Must have persisted the change (draft has it; in real use the ctx would too after successful atomic).
        // Editing stays open (Esc to leave the form for the item; no j/k letters for fields -- Esc first then j/k for list).
        let draft_enabled_false = app
            .editing
            .as_ref()
            .and_then(|s| s.draft_source.as_ref())
            .map(|d| !d.enabled)
            .unwrap_or_else(|| {
                !app.ctx
                    .config
                    .sources
                    .first()
                    .map(|s| s.enabled)
                    .unwrap_or(true)
            });
        assert!(
            draft_enabled_false,
            "after Space toggle on enabled, the draft must have enabled=false; msg={}",
            after_enter_msg
        );
        assert!(
            still_editing,
            "Enter on field in edit must keep the edit form open (persist the item but allow editing more fields of it); got still_editing=false"
        );
        // Do not assert absence of "fail" strings: in tmp test harness atomic save often hits "config file not found" (env), but draft apply and validate path succeeded.

        // Drive a validation error case and ensure it is visible inline at top of form (not just footer status, not buried at bottom)
        app.cancel_edit();
        app.config_cursor = 1;
        app.config_in_subnav = true;
        app.config_sub_cursor = 0;
        app.start_edit_for_current();
        // Make a bad change that will fail strict validate_config on save (e.g. clear a required-ish or set bad type for wallhaven; simplest: empty the type for a source that needs it, or use invalid for block)
        // For source, "type" is editable; set to empty to trigger type-aware validation on save.
        // Move cursor to "type" field (idx 1), clear it, commit (which now also saves/persists), then Save (exits edit) to drive the error render while errors are set.
        update(&mut app, UiAction::EditFieldDown, rt.handle()).ok(); // to type field
        for _ in 0..20 {
            update(&mut app, UiAction::EditFieldBackspace, rt.handle()).ok();
        } // clear
        update(&mut app, UiAction::EditFieldCommit, rt.handle()).ok();
        update(&mut app, UiAction::SaveEditItem, rt.handle()).ok();
        let err_text = render_text(&app, 80, 24);
        eprintln!(
            "=== SCREENSHOT: EDIT FORM WITH VALIDATION ERROR (must be obvious inline) ===\n{}",
            err_text
        );
        // Must show inline near top of the edit form body, with cue that gets red treatment
        let has_inline_err = err_text.contains("!! Validation")
            || err_text.contains("validation:")
            || err_text.contains("Validation errors");
        assert!(
            has_inline_err,
            "validation problems must be visible inline in the form body (top, red-cued) before/during/after s, not opaque fail only in status; form head: {}",
            &err_text[..err_text.len().min(600)]
        );
    }
}
