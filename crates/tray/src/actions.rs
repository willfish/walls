//! Tray menu actions — must not block inside ksni dbus callbacks.

use std::path::PathBuf;

use crate::rotation;
use crate::{resolve_walls_bin, run_walls, WallsCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Next,
    Prev,
    TogglePause,
    OpenTui,
    Quit,
}

pub fn dispatch(action: MenuAction) {
    let walls = resolve_walls_bin();
    match action {
        MenuAction::Next => rotation::advance_manual(),
        MenuAction::Prev => run_command(&walls, WallsCommand::Prev, "prev"),
        MenuAction::TogglePause => run_command(&walls, WallsCommand::TogglePause, "toggle-pause"),
        MenuAction::OpenTui => {
            if let Err(err) = crate::tui::spawn_tui(&walls) {
                tracing::warn!("open TUI failed: {err:#}");
            }
        }
        MenuAction::Quit => {}
    }
}

fn run_command(walls: &PathBuf, cmd: WallsCommand, label: &str) {
    if let Err(err) = run_walls(walls, cmd.args()) {
        tracing::warn!("walls {label} failed: {err:#}");
    }
}
