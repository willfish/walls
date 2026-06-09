//! Tray menu actions — must not block inside ksni dbus callbacks.

use std::path::PathBuf;

use crate::rotation;
use crate::{resolve_walls_bin, run_walls, WallsCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Next,
    Prev,
    Favorite,
    TogglePause,
    OpenTui,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuActionSpec {
    pub action: MenuAction,
    pub label: &'static str,
    pub separator_before: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionOutcome {
    pub refresh: bool,
    pub quit: bool,
}

const MENU_ACTIONS: &[MenuActionSpec] = &[
    MenuActionSpec {
        action: MenuAction::Next,
        label: "Next wallpaper",
        separator_before: false,
    },
    MenuActionSpec {
        action: MenuAction::Prev,
        label: "Previous wallpaper",
        separator_before: false,
    },
    MenuActionSpec {
        action: MenuAction::Favorite,
        label: "Favorite current wallpaper",
        separator_before: false,
    },
    MenuActionSpec {
        action: MenuAction::TogglePause,
        label: "Toggle pause",
        separator_before: false,
    },
    MenuActionSpec {
        action: MenuAction::OpenTui,
        label: "Open TUI",
        separator_before: false,
    },
    MenuActionSpec {
        action: MenuAction::Quit,
        label: "Quit tray",
        separator_before: true,
    },
];

impl MenuAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Next => "Next wallpaper",
            Self::Prev => "Previous wallpaper",
            Self::Favorite => "Favorite current wallpaper",
            Self::TogglePause => "Toggle pause",
            Self::OpenTui => "Open TUI",
            Self::Quit => "Quit tray",
        }
    }

    pub fn command(self) -> Option<WallsCommand> {
        match self {
            Self::Prev => Some(WallsCommand::Prev),
            Self::Favorite => Some(WallsCommand::Favorite),
            Self::TogglePause => Some(WallsCommand::TogglePause),
            Self::Next | Self::OpenTui | Self::Quit => None,
        }
    }

    pub fn outcome(self) -> ActionOutcome {
        match self {
            Self::Next | Self::Prev | Self::Favorite | Self::TogglePause => ActionOutcome {
                refresh: true,
                quit: false,
            },
            Self::OpenTui => ActionOutcome {
                refresh: false,
                quit: false,
            },
            Self::Quit => ActionOutcome {
                refresh: false,
                quit: true,
            },
        }
    }
}

pub fn menu_actions() -> &'static [MenuActionSpec] {
    MENU_ACTIONS
}

pub fn dispatch(action: MenuAction) -> ActionOutcome {
    let walls = resolve_walls_bin();
    match action {
        MenuAction::Next => rotation::advance_manual(),
        MenuAction::Prev | MenuAction::Favorite | MenuAction::TogglePause => {
            if let Some(command) = action.command() {
                run_command(&walls, command, action.label());
            }
        }
        MenuAction::OpenTui => {
            if let Err(err) = crate::tui::spawn_tui(&walls) {
                tracing::warn!("open TUI failed: {err:#}");
            }
        }
        MenuAction::Quit => {}
    }
    action.outcome()
}

fn run_command(walls: &PathBuf, cmd: WallsCommand, label: &str) {
    if let Err(err) = run_walls(walls, cmd.args()) {
        tracing::warn!("walls {label} failed: {err:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_actions_define_shared_order_labels_and_separator() {
        let specs = menu_actions();
        let actions = specs.iter().map(|spec| spec.action).collect::<Vec<_>>();
        assert_eq!(
            actions,
            vec![
                MenuAction::Next,
                MenuAction::Prev,
                MenuAction::Favorite,
                MenuAction::TogglePause,
                MenuAction::OpenTui,
                MenuAction::Quit
            ]
        );
        assert_eq!(specs[0].label, "Next wallpaper");
        assert_eq!(specs[1].label, "Previous wallpaper");
        assert_eq!(specs[2].label, "Favorite current wallpaper");
        assert_eq!(specs[3].label, "Toggle pause");
        assert_eq!(specs[4].label, "Open TUI");
        assert_eq!(specs[5].label, "Quit tray");
        assert!(specs[5].separator_before);
        assert!(specs[..5].iter().all(|spec| !spec.separator_before));
    }

    #[test]
    fn menu_actions_expose_command_mapping_and_outcomes() {
        assert_eq!(MenuAction::Next.command(), None);
        assert_eq!(MenuAction::Prev.command(), Some(WallsCommand::Prev));
        assert_eq!(MenuAction::Favorite.command(), Some(WallsCommand::Favorite));
        assert_eq!(
            MenuAction::TogglePause.command(),
            Some(WallsCommand::TogglePause)
        );
        assert_eq!(MenuAction::OpenTui.command(), None);
        assert_eq!(MenuAction::Quit.command(), None);

        assert_eq!(
            MenuAction::Next.outcome(),
            ActionOutcome {
                refresh: true,
                quit: false
            }
        );
        assert_eq!(
            MenuAction::Favorite.outcome(),
            ActionOutcome {
                refresh: true,
                quit: false
            }
        );
        assert_eq!(
            MenuAction::Quit.outcome(),
            ActionOutcome {
                refresh: false,
                quit: true
            }
        );
    }

    #[test]
    fn run_command_logs_failures_without_panicking() {
        let exe = std::env::current_exe().expect("current test binary");

        assert!(run_walls(&exe, &["--definitely-not-a-libtest-flag"]).is_err());
        run_command(&exe, WallsCommand::Favorite, "favorite");
        run_command(&exe, WallsCommand::Prev, "previous wallpaper");
    }
}
