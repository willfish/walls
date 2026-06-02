mod app;

use std::io::{stdout, IsTerminal};

use anyhow::Context;
use app::{App, Tab};
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs};
use walls_core::WallsCtx;

/// Blocking TUI loop. Uses the `#[tokio::main]` runtime via [`Handle::current`].
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

    let mut app = App::new(
        WallsCtx::load().context(
            "failed to load ~/.config/walls/config.json — copy config.example.json to get started",
        )?,
    )?;

    loop {
        terminal.draw(|f| draw(f, &app))?;
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
             Try: walls tui   # from a terminal emulator, not a pipe or IDE task output"
        );
    }
    Ok(())
}

/// Restore terminal state even when the loop errors or panics.
struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
    }
}

fn handle_key(
    app: &mut App,
    key: KeyEvent,
    rt: &tokio::runtime::Handle,
) -> anyhow::Result<bool> {
    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('n') | KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {}
        KeyCode::Char('n') => {
            app.message = match rt.block_on(app.ctx.advance_next()) {
                Ok(Some(p)) => format!("next: {}", p.display()),
                Ok(None) => "next: no change".into(),
                Err(e) => format!("next error: {e}"),
            };
            app.reload_ctx()?;
        }
        KeyCode::Char('p') => {
            app.message = match app.ctx.advance_prev() {
                Ok(Some(p)) => format!("prev: {}", p.display()),
                Ok(None) => "prev: none".into(),
                Err(e) => format!("prev error: {e}"),
            };
            app.reload_ctx()?;
        }
        KeyCode::Char(' ') => {
            let paused = !app.ctx.state.paused;
            app.ctx.state.paused = paused;
            app.ctx.save_state()?;
            app.message = format!("paused: {paused}");
        }
        KeyCode::Char('1') => app.tab = Tab::Status,
        KeyCode::Char('2') => app.tab = Tab::Now,
        KeyCode::Char('3') => {
            app.tab = Tab::History;
            app.cursor = 0;
        }
        KeyCode::Char('4') => {
            app.tab = Tab::Browse;
            app.cursor = 0;
        }
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Enter => {
            if app.tab == Tab::History {
                if let Some(path) = app.apply_history_selection() {
                    app.message = format!("applied: {}", path.display());
                    app.reload_ctx()?;
                }
            } else if app.tab == Tab::Browse {
                if let Some(msg) = rt.block_on(app.apply_browse_selection())? {
                    app.message = msg;
                    app.reload_ctx()?;
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    if area.height < 6 || area.width < 10 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let titles = vec!["Status", "Now", "History", "Browse"];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("walls"))
        .select(app.tab.index());
    f.render_widget(tabs, chunks[0]);

    let body = match app.tab {
        Tab::Status => status_lines(app),
        Tab::Now => now_lines(app),
        Tab::History => app.history_lines(),
        Tab::Browse => app.browse_lines(),
    };
    let items: Vec<ListItem> = body.iter().map(|l| ListItem::new(l.as_str())).collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(app.tab.title()));
    f.render_widget(list, chunks[1]);

    let help = Paragraph::new(app.footer_help())
        .block(Block::default().borders(Borders::ALL).title("keys"));
    f.render_widget(help, chunks[2]);
}

fn status_lines(app: &App) -> Vec<String> {
    vec![
        format!("paused: {}", app.ctx.state.paused),
        format!("change enabled: {}", app.ctx.config.change.enabled),
        format!("config: {}", app.ctx.paths.config_file.display()),
        format!("state: {}", app.ctx.paths.state_file.display()),
        format!("history: {} entries", app.ctx.state.history.len()),
        format!("cache queue: {} ids", app.ctx.state.cache_queue.len()),
        format!("cache dir: {}", app.ctx.paths.cache_dir.display()),
        app.message.clone(),
    ]
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