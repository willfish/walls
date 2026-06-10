//! Tray menu actions — must not block inside ksni dbus callbacks.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::resolve_walls_bin;
use crate::rotation;
use walls_core::rotation::any_sources_enabled;
use walls_core::WallsCtx;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Next,
    Prev,
    Favorite,
    TogglePause,
    OpenTui,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuActionSpec {
    pub action: MenuAction,
    pub label: Cow<'static, str>,
    pub separator_before: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuLabelState {
    pub paused: Option<bool>,
    pub rotation_enabled: Option<bool>,
    pub active_sources: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFeedback {
    pub kind: FeedbackKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackKind {
    Success,
    NoChange,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionOutcome {
    pub refresh: bool,
    pub quit: bool,
    pub feedback: Option<ActionFeedback>,
}

impl MenuAction {
    pub fn label(self) -> Cow<'static, str> {
        self.label_for_state(MenuLabelState::default())
    }

    pub fn label_for_state(self, state: MenuLabelState) -> Cow<'static, str> {
        match self {
            Self::Next => "Next wallpaper".into(),
            Self::Prev => "Previous wallpaper".into(),
            Self::Favorite => "Favorite current wallpaper".into(),
            Self::TogglePause => match (state.paused, state.active_sources, state.rotation_enabled)
            {
                (Some(true), _, _) => "Resume rotation".into(),
                (Some(false), Some(false), _) => "Pause rotation (no active sources)".into(),
                (Some(false), _, Some(false)) => "Pause rotation (rotation disabled)".into(),
                (Some(false), _, _) => "Pause rotation".into(),
                _ => "Toggle pause".into(),
            },
            Self::OpenTui => "Open TUI".into(),
            Self::Quit => "Quit tray".into(),
        }
    }

    pub fn outcome(self) -> ActionOutcome {
        match self {
            Self::Next | Self::Prev | Self::Favorite | Self::TogglePause => {
                ActionOutcome::refreshing(None)
            }
            Self::OpenTui => ActionOutcome {
                refresh: false,
                quit: false,
                feedback: None,
            },
            Self::Quit => ActionOutcome {
                refresh: false,
                quit: true,
                feedback: None,
            },
        }
    }
}

impl ActionOutcome {
    fn refreshing(feedback: Option<ActionFeedback>) -> Self {
        Self {
            refresh: true,
            quit: false,
            feedback,
        }
    }
}

impl ActionFeedback {
    fn success(message: impl Into<String>) -> Self {
        Self {
            kind: FeedbackKind::Success,
            message: message.into(),
        }
    }

    fn no_change(message: impl Into<String>) -> Self {
        Self {
            kind: FeedbackKind::NoChange,
            message: message.into(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            kind: FeedbackKind::Error,
            message: message.into(),
        }
    }
}

pub fn menu_actions() -> Vec<MenuActionSpec> {
    menu_actions_for_state(load_menu_label_state())
}

pub fn menu_actions_for_state(state: MenuLabelState) -> Vec<MenuActionSpec> {
    [
        (MenuAction::Next, false),
        (MenuAction::Prev, false),
        (MenuAction::Favorite, false),
        (MenuAction::TogglePause, false),
        (MenuAction::OpenTui, false),
        (MenuAction::Quit, true),
    ]
    .into_iter()
    .map(|(action, separator_before)| MenuActionSpec {
        action,
        label: action.label_for_state(state),
        separator_before,
    })
    .collect()
}

pub fn dispatch(action: MenuAction) -> ActionOutcome {
    let walls = resolve_walls_bin();
    match action {
        MenuAction::Next => dispatch_next(),
        MenuAction::Prev => dispatch_prev(),
        MenuAction::Favorite => dispatch_favorite(),
        MenuAction::TogglePause => dispatch_toggle_pause(),
        MenuAction::OpenTui => match crate::tui::spawn_tui(&walls) {
            Ok(()) => ActionOutcome {
                refresh: false,
                quit: false,
                feedback: Some(ActionFeedback::success("Opened TUI")),
            },
            Err(err) => {
                let message = open_tui_error_message(&err);
                tracing::warn!("{message}");
                ActionOutcome {
                    refresh: true,
                    quit: false,
                    feedback: Some(ActionFeedback::error(message)),
                }
            }
        },
        MenuAction::Quit => ActionOutcome {
            refresh: false,
            quit: true,
            feedback: None,
        },
    }
}

fn open_tui_error_message(error: &anyhow::Error) -> String {
    format!(
        "Open TUI failed: {error:#}. Run `walls tui` directly or set WALLS_TUI_CMD, for example `ghostty --class=walls -e {{walls}} tui`."
    )
}

pub fn tooltip_with_feedback(base: &str, feedback: Option<&ActionFeedback>) -> String {
    match feedback {
        Some(feedback) => format!("{base} - {}", sanitize_message(&feedback.message)),
        None => base.into(),
    }
}

fn dispatch_next() -> ActionOutcome {
    match rotation::advance_manual() {
        Ok(Some(path)) => ActionOutcome::refreshing(Some(ActionFeedback::success(format!(
            "Applied next wallpaper: {}",
            display_path(&path)
        )))),
        Ok(None) => ActionOutcome::refreshing(Some(ActionFeedback::no_change(
            "No next wallpaper available",
        ))),
        Err(err) => {
            let message = format!("Next wallpaper failed: {err:#}");
            tracing::warn!("{message}");
            ActionOutcome::refreshing(Some(ActionFeedback::error(message)))
        }
    }
}

fn dispatch_prev() -> ActionOutcome {
    let result: anyhow::Result<Option<PathBuf>> = (|| {
        let mut ctx = WallsCtx::load()?;
        Ok(ctx.advance_prev()?)
    })();
    match result {
        Ok(Some(path)) => ActionOutcome::refreshing(Some(ActionFeedback::success(format!(
            "Restored previous wallpaper: {}",
            display_path(&path)
        )))),
        Ok(None) => {
            ActionOutcome::refreshing(Some(ActionFeedback::no_change("No previous wallpaper")))
        }
        Err(err) => {
            let message = format!("Previous wallpaper failed: {err:#}");
            tracing::warn!("{message}");
            ActionOutcome::refreshing(Some(ActionFeedback::error(message)))
        }
    }
}

fn dispatch_favorite() -> ActionOutcome {
    let result: anyhow::Result<PathBuf> = (|| {
        let ctx = WallsCtx::load()?;
        ctx.favorite_current()
    })();
    match result {
        Ok(path) => ActionOutcome::refreshing(Some(ActionFeedback::success(format!(
            "Favorited current wallpaper: {}",
            display_path(&path)
        )))),
        Err(err) => {
            let message = format!("Favorite current wallpaper failed: {err:#}");
            tracing::warn!("{message}");
            ActionOutcome::refreshing(Some(ActionFeedback::error(message)))
        }
    }
}

fn dispatch_toggle_pause() -> ActionOutcome {
    let result: anyhow::Result<bool> = (|| {
        let mut ctx = WallsCtx::load()?;
        ctx.toggle_pause()?;
        Ok(ctx.state.paused)
    })();
    match result {
        Ok(true) => ActionOutcome::refreshing(Some(ActionFeedback::success("Rotation paused"))),
        Ok(false) => ActionOutcome::refreshing(Some(ActionFeedback::success("Rotation resumed"))),
        Err(err) => {
            let message = format!("Toggle pause failed: {err:#}");
            tracing::warn!("{message}");
            ActionOutcome::refreshing(Some(ActionFeedback::error(message)))
        }
    }
}

fn load_menu_label_state() -> MenuLabelState {
    let Ok(ctx) = WallsCtx::load() else {
        return MenuLabelState::default();
    };
    MenuLabelState {
        paused: Some(ctx.state.paused),
        rotation_enabled: Some(ctx.config.change.enabled),
        active_sources: Some(any_sources_enabled(&ctx.config)),
    }
}

fn display_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| path.display().to_string(), ToOwned::to_owned)
}

fn sanitize_message(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_actions_define_shared_order_labels_and_separator() {
        let specs = menu_actions_for_state(MenuLabelState {
            paused: None,
            ..MenuLabelState::default()
        });
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
    fn menu_actions_make_pause_label_state_aware() {
        assert_eq!(
            menu_actions_for_state(MenuLabelState {
                paused: Some(false),
                rotation_enabled: Some(true),
                active_sources: Some(true),
            })[3]
                .label,
            "Pause rotation"
        );
        assert_eq!(
            menu_actions_for_state(MenuLabelState {
                paused: Some(true),
                rotation_enabled: Some(true),
                active_sources: Some(true),
            })[3]
                .label,
            "Resume rotation"
        );
    }

    #[test]
    fn menu_actions_explain_derived_inactive_rotation() {
        assert_eq!(
            menu_actions_for_state(MenuLabelState {
                paused: Some(true),
                rotation_enabled: Some(true),
                active_sources: Some(false),
            })[3]
                .label,
            "Resume rotation"
        );
        assert_eq!(
            menu_actions_for_state(MenuLabelState {
                paused: Some(false),
                rotation_enabled: Some(false),
                active_sources: Some(true),
            })[3]
                .label,
            "Pause rotation (rotation disabled)"
        );
        assert_eq!(
            menu_actions_for_state(MenuLabelState {
                paused: Some(false),
                rotation_enabled: Some(true),
                active_sources: Some(false),
            })[3]
                .label,
            "Pause rotation (no active sources)"
        );
    }

    #[test]
    fn menu_actions_expose_outcomes() {
        assert_eq!(
            MenuAction::Next.outcome(),
            ActionOutcome {
                refresh: true,
                quit: false,
                feedback: None
            }
        );
        assert_eq!(
            MenuAction::Favorite.outcome(),
            ActionOutcome {
                refresh: true,
                quit: false,
                feedback: None
            }
        );
        assert_eq!(
            MenuAction::Quit.outcome(),
            ActionOutcome {
                refresh: false,
                quit: true,
                feedback: None
            }
        );
    }

    #[test]
    fn tooltip_feedback_sanitizes_failure_messages() {
        let feedback = ActionFeedback::error("Previous wallpaper failed:\nno previous wallpaper");

        assert_eq!(
            tooltip_with_feedback("walls", Some(&feedback)),
            "walls - Previous wallpaper failed: no previous wallpaper"
        );
        assert_eq!(tooltip_with_feedback("walls", None), "walls");
    }

    #[test]
    fn open_tui_error_message_includes_recovery() {
        let error = anyhow::anyhow!(
            "failed to launch TUI via TERMINAL: ghostty --class=walls -e /opt/walls/bin/walls tui"
        )
        .context("terminal not found");
        let message = open_tui_error_message(&error);

        assert!(message.contains("terminal not found"), "{message}");
        assert!(message.contains("via TERMINAL"), "{message}");
        assert!(message.contains("ghostty --class=walls"), "{message}");
        assert!(message.contains("walls tui"), "{message}");
        assert!(message.contains("WALLS_TUI_CMD"), "{message}");
    }

    #[test]
    fn display_path_prefers_file_name_for_short_feedback() {
        assert_eq!(display_path(Path::new("/tmp/walls/wall.jpg")), "wall.jpg");
        assert_eq!(display_path(Path::new("/")), "/");
    }
}
