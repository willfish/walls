mod app;
mod chrome_view;
mod command;
mod config_detail_view;
mod config_edit_view;
mod config_view;
mod history_browse_view;
mod input_update;
mod layout_size;
mod line_view;
mod logs_view;
mod main_view;
mod now_view;
mod open_target;
#[cfg(feature = "tui-preview")]
mod preview;
mod runtime;
mod search_view;
mod sources_view;
mod startup;
mod style;

use anyhow::Context;
use app::App;
#[cfg(test)]
pub(crate) use app::{InputMode, Tab};
#[cfg(test)]
pub(crate) use app::{
    CONFIG_BLOCK_APPLY_DISPLAY, CONFIG_BLOCK_LIBRARY, CONFIG_BLOCK_ROTATION, CONFIG_BLOCK_SOURCES,
    CONFIG_BLOCK_TUI,
};
use input_update::handle_key;
#[cfg(test)]
pub(crate) use input_update::{action_for_key, apply_effect, update, UiAction, UpdateEffect};
#[cfg(test)]
pub(crate) use layout_size::{terminal_size, TerminalSize};
#[cfg(test)]
pub(crate) use main_view::draw_inner;
#[cfg(all(test, feature = "tui-preview"))]
pub(crate) use main_view::selected_preview_path;
use ratatui::crossterm::event::{self, Event};
use ratatui::prelude::*;
pub(crate) use runtime::{log_len, CaptureWriter, ConsoleWriter, LOG_BUFFER};
use startup::{draw_startup_intro, start_intro_preview_prewarm, StartupIntro};
use walls_core::WallsCtx;

pub fn run(startup_message: Option<String>, tray_owns_rotation: bool) -> anyhow::Result<()> {
    let rt = tokio::runtime::Handle::current();

    let (mut terminal, _restore) = runtime::enter_terminal()?;

    let mut app = App::new(WallsCtx::load().context("failed to load walls config")?)?;
    if let Some(message) = startup_message {
        app.set_message(style::StatusKind::Neutral, message);
    }
    let mut startup_intro = StartupIntro::from_env();
    let _intro_prewarm = start_intro_preview_prewarm(&app, startup_intro.is_active());
    runtime::mark_in_tui();
    #[cfg(feature = "tui-preview")]
    let mut preview = preview::ImagePreview::detect();
    let mut auto_rotator = if tray_owns_rotation {
        None
    } else {
        Some(walls_core::rotation::AutoRotator::new())
    };

    loop {
        app.sync_log_cursor();
        terminal.draw(|f| {
            if startup_intro.is_active() {
                draw_startup_intro(f, &app, &startup_intro);
            } else {
                #[cfg(feature = "tui-preview")]
                draw(f, &app, &mut preview);
                #[cfg(not(feature = "tui-preview"))]
                draw(f, &app);
            }
        })?;
        if !startup_intro.is_active() {
            if let Some(rotator) = &mut auto_rotator {
                let outcome = tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        let mut ctx = walls_core::WallsCtx::load()?;
                        Ok::<_, anyhow::Error>(rotator.tick(&mut ctx).await)
                    })
                });
                if matches!(outcome, Ok(walls_core::rotation::TickOutcome::Rotated)) {
                    app.reload_ctx()?;
                }
            }
        }
        if event::poll(startup_intro.poll_interval())? {
            if let Event::Key(key) = event::read()? {
                startup_intro.skip();
                if handle_key(&mut app, key, &rt)? {
                    break;
                }
            }
        } else {
            startup_intro.tick();
        }
    }

    Ok(())
}

#[cfg(feature = "tui-preview")]
fn draw(f: &mut Frame, app: &App, preview: &mut preview::ImagePreview) {
    main_view::draw_inner(f, app, Some(preview));
}

#[cfg(not(feature = "tui-preview"))]
fn draw(f: &mut Frame, app: &App) {
    main_view::draw_inner(f, app);
}

#[cfg(test)]
mod tests;
