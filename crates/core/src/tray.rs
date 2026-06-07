//! Tray auto-start capability.
//!
//! On Wayland, `walls-tray` uses `StatusNotifierItem` (D-Bus) — the protocol COSMIC and KDE
//! expose via `org.kde.StatusNotifierWatcher`. On X11 it falls back to `AppIndicator`.

use crate::apply::{detect_desktop_from_env, Desktop};

/// Graphical session transport from `XDG_SESSION_TYPE` / display env.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    Wayland,
    X11,
    Unknown,
}

/// Whether the TUI should attempt to spawn `walls-tray` on launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    /// Spawn `walls-tray` when none is running (TUI does not replace an existing tray).
    Spawn,
    /// Do not spawn; `reason` is user-facing.
    Skip { reason: String },
}

/// Parse `XDG_SESSION_TYPE` (and related hints) into a session type.
pub fn session_type_from_env(
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> SessionType {
    if let Some(kind) = xdg_session_type {
        let lower = kind.to_ascii_lowercase();
        if lower == "wayland" {
            return SessionType::Wayland;
        }
        if lower == "x11" || lower == "xorg" {
            return SessionType::X11;
        }
    }
    if wayland_display.is_some_and(|v| !v.is_empty()) {
        return SessionType::Wayland;
    }
    if display.is_some_and(|v| !v.is_empty()) {
        return SessionType::X11;
    }
    SessionType::Unknown
}

fn has_graphical_session(session: SessionType) -> bool {
    !matches!(session, SessionType::Unknown)
}

/// Decide tray action from environment (testable).
pub fn decide_tray_action_from_env(
    walls_tray: Option<&str>,
    xdg_current_desktop: Option<&str>,
    xdg_session_desktop: Option<&str>,
    desktop_startup_id: Option<&str>,
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> TrayAction {
    if walls_tray.is_some_and(|v| matches!(v, "0" | "false" | "no" | "off")) {
        return TrayAction::Skip {
            reason: "tray disabled (WALLS_TRAY=0)".into(),
        };
    }

    let force = walls_tray.is_some_and(|v| matches!(v, "1" | "true" | "yes" | "force"));
    let desktop =
        detect_desktop_from_env(xdg_current_desktop, xdg_session_desktop, desktop_startup_id);
    let session = session_type_from_env(xdg_session_type, wayland_display, display);

    if !force {
        if !has_graphical_session(session) {
            return TrayAction::Skip {
                reason: "no graphical session (tray needs Wayland or X11)".into(),
            };
        }

        if let Some(reason) = unsupported_tray_reason(desktop) {
            return TrayAction::Skip { reason };
        }
    }

    TrayAction::Spawn
}

/// Live environment lookup.
pub fn decide_tray_action() -> TrayAction {
    decide_tray_action_from_env(
        std::env::var("WALLS_TRAY").ok().as_deref(),
        std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
        std::env::var("XDG_SESSION_DESKTOP").ok().as_deref(),
        std::env::var("DESKTOP_STARTUP_ID").ok().as_deref(),
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
    )
}

fn unsupported_tray_reason(desktop: Desktop) -> Option<String> {
    match desktop {
        Desktop::Enlightenment
        | Desktop::Awesome
        | Desktop::Fluxbox
        | Desktop::Trinity
        | Desktop::Lingmo => Some(format!(
            "{} is not supported for walls-tray yet; use the TUI",
            desktop_name(desktop)
        )),
        _ => None,
    }
}

fn desktop_name(desktop: Desktop) -> &'static str {
    match desktop {
        Desktop::Gnome => "GNOME",
        Desktop::Unity => "Unity",
        Desktop::Budgie => "Budgie",
        Desktop::Kde => "KDE",
        Desktop::Xfce => "XFCE",
        Desktop::Lxde => "LXDE",
        Desktop::Lxqt => "LXQt",
        Desktop::Mate => "MATE",
        Desktop::Cinnamon => "Cinnamon",
        Desktop::Lingmo => "Lingmo",
        Desktop::Deepin => "Deepin",
        Desktop::Trinity => "Trinity",
        Desktop::Fluxbox => "Fluxbox",
        Desktop::Sway => "Sway",
        Desktop::Hyprland => "Hyprland",
        Desktop::Enlightenment => "Enlightenment",
        Desktop::Awesome => "Awesome",
        Desktop::Cosmic => "COSMIC",
        Desktop::Unknown => "this desktop",
    }
}
