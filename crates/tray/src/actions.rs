//! Tray menu actions — must not block inside ksni dbus callbacks.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::resolve_walls_bin;
use crate::rotation;
use walls_core::rotation::RotationAvailability;
use walls_core::WallsCtx;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Next,
    Prev,
    Favorite,
    Pause,
    Resume,
    OpenTui,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuActionSpec {
    pub action: Option<MenuAction>,
    pub label: Cow<'static, str>,
    pub separator_before: bool,
    pub enabled: bool,
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
        self.label_text()
    }

    pub fn label_text(self) -> Cow<'static, str> {
        match self {
            Self::Next => "Next wallpaper".into(),
            Self::Prev => "Previous wallpaper".into(),
            Self::Favorite => "Favorite current wallpaper".into(),
            Self::Pause => "Pause rotation".into(),
            Self::Resume => "Resume rotation".into(),
            Self::OpenTui => "Open TUI".into(),
            Self::Quit => "Quit tray".into(),
        }
    }

    pub fn outcome(self) -> ActionOutcome {
        match self {
            Self::Next | Self::Prev | Self::Favorite | Self::Pause | Self::Resume => {
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
    menu_actions_for_availability(load_rotation_availability())
}

pub fn menu_actions_for_availability(
    availability: Option<RotationAvailability>,
) -> Vec<MenuActionSpec> {
    let pause_action = match availability {
        Some(state) if state.paused => MenuAction::Resume,
        Some(_) => MenuAction::Pause,
        None => MenuAction::Pause,
    };
    let mut specs = [
        (MenuAction::Next, false),
        (MenuAction::Prev, false),
        (MenuAction::Favorite, false),
        (pause_action, false),
        (MenuAction::OpenTui, false),
        (MenuAction::Quit, true),
    ]
    .into_iter()
    .map(|(action, separator_before)| MenuActionSpec {
        action: Some(action),
        label: action.label_text(),
        separator_before,
        enabled: true,
    })
    .collect::<Vec<_>>();

    if availability.is_some_and(|state| !state.active_sources) {
        let quit_index = specs
            .iter()
            .position(|spec| spec.action == Some(MenuAction::Quit))
            .unwrap_or(specs.len());
        specs.insert(
            quit_index,
            MenuActionSpec {
                action: None,
                label: "No active Sources".into(),
                separator_before: true,
                enabled: false,
            },
        );
    }

    specs
}

pub fn dispatch(action: MenuAction) -> ActionOutcome {
    let walls = resolve_walls_bin();
    match action {
        MenuAction::Next => dispatch_next(),
        MenuAction::Prev => dispatch_prev(),
        MenuAction::Favorite => dispatch_favorite(),
        MenuAction::Pause => dispatch_set_paused(true),
        MenuAction::Resume => dispatch_set_paused(false),
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

fn dispatch_set_paused(paused: bool) -> ActionOutcome {
    let result: anyhow::Result<bool> = (|| {
        let mut ctx = WallsCtx::load()?;
        ctx.set_paused(paused)?;
        Ok(ctx.state.paused)
    })();
    match result {
        Ok(true) => ActionOutcome::refreshing(Some(ActionFeedback::success("Rotation paused"))),
        Ok(false) => ActionOutcome::refreshing(Some(ActionFeedback::success("Rotation resumed"))),
        Err(err) => {
            let action = if paused { "Pause" } else { "Resume" };
            let message = format!("{action} rotation failed: {err:#}");
            tracing::warn!("{message}");
            ActionOutcome::refreshing(Some(ActionFeedback::error(message)))
        }
    }
}

fn load_rotation_availability() -> Option<RotationAvailability> {
    let Ok(ctx) = WallsCtx::load() else {
        return None;
    };
    Some(RotationAvailability::from_state_config(
        &ctx.state,
        &ctx.config,
    ))
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

    fn availability(
        paused: bool,
        change_enabled: bool,
        active_sources: bool,
    ) -> RotationAvailability {
        RotationAvailability {
            paused,
            change_enabled,
            active_sources,
        }
    }

    #[test]
    fn menu_actions_define_shared_order_labels_and_separator() {
        let specs = menu_actions_for_availability(None);
        let actions = specs
            .iter()
            .filter_map(|spec| spec.action)
            .collect::<Vec<_>>();
        assert_eq!(
            actions,
            vec![
                MenuAction::Next,
                MenuAction::Prev,
                MenuAction::Favorite,
                MenuAction::Pause,
                MenuAction::OpenTui,
                MenuAction::Quit
            ]
        );
        assert_eq!(specs[0].label, "Next wallpaper");
        assert_eq!(specs[1].label, "Previous wallpaper");
        assert_eq!(specs[2].label, "Favorite current wallpaper");
        assert_eq!(specs[3].label, "Pause rotation");
        assert_eq!(specs[4].label, "Open TUI");
        assert_eq!(specs[5].label, "Quit tray");
        assert!(specs[5].separator_before);
        assert!(specs[..5].iter().all(|spec| !spec.separator_before));
        assert!(specs.iter().all(|spec| spec.enabled));
    }

    #[test]
    fn menu_actions_choose_explicit_pause_or_resume_action() {
        let pause = menu_actions_for_availability(Some(availability(false, true, true)));
        assert_eq!(pause[3].action, Some(MenuAction::Pause));
        assert_eq!(pause[3].label, "Pause rotation");

        let resume = menu_actions_for_availability(Some(availability(true, true, true)));
        assert_eq!(resume[3].action, Some(MenuAction::Resume));
        assert_eq!(resume[3].label, "Resume rotation");
    }

    #[test]
    fn pause_label_only_reflects_user_pause_state() {
        let paused_without_sources =
            menu_actions_for_availability(Some(availability(true, true, false)));
        assert_eq!(paused_without_sources[3].action, Some(MenuAction::Resume));
        assert_eq!(paused_without_sources[3].label, "Resume rotation");

        let rotation_disabled =
            menu_actions_for_availability(Some(availability(false, false, true)));
        assert_eq!(rotation_disabled[3].action, Some(MenuAction::Pause));
        assert_eq!(rotation_disabled[3].label, "Pause rotation");

        let no_sources = menu_actions_for_availability(Some(availability(false, true, false)));
        assert_eq!(no_sources[3].action, Some(MenuAction::Pause));
        assert_eq!(no_sources[3].label, "Pause rotation");
    }

    #[test]
    fn menu_actions_show_no_active_sources_status_only_when_needed() {
        let with_sources = menu_actions_for_availability(Some(availability(false, true, true)));
        assert!(!with_sources.iter().any(|spec| spec.action.is_none()));
        assert_eq!(
            with_sources
                .iter()
                .filter(|spec| spec.separator_before)
                .count(),
            1
        );

        let without_sources = menu_actions_for_availability(Some(availability(false, true, false)));
        let status = without_sources
            .iter()
            .find(|spec| spec.action.is_none())
            .expect("no active sources status item");

        assert_eq!(status.label, "No active Sources");
        assert!(status.separator_before);
        assert!(!status.enabled);
        assert_eq!(
            without_sources
                .iter()
                .position(|spec| spec.action.is_none()),
            without_sources
                .iter()
                .position(|spec| spec.action == Some(MenuAction::Quit))
                .map(|index| index.saturating_sub(1))
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
