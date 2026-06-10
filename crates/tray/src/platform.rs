//! Linux tray host detection (StatusNotifier vs legacy AppIndicator).

use walls_core::tray::session_type_from_env;

/// Whether the session is Wayland (SNI is the native tray path).
pub fn is_wayland_session() -> bool {
    is_wayland_session_from_env(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
    )
}

/// Prefer StatusNotifierItem on Wayland; AppIndicator remains the X11 path.
pub fn prefer_status_notifier() -> bool {
    is_wayland_session()
}

fn is_wayland_session_from_env(
    session_type: Option<&str>,
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> bool {
    matches!(
        session_type_from_env(session_type, wayland_display, display),
        walls_core::tray::SessionType::Wayland
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_wayland_from_session_type() {
        assert!(is_wayland_session_from_env(
            Some("wayland"),
            None,
            Some(":0")
        ));
    }

    #[test]
    fn detects_wayland_from_wayland_display() {
        assert!(is_wayland_session_from_env(
            None,
            Some("wayland-0"),
            Some(":0")
        ));
    }

    #[test]
    fn treats_x11_display_as_not_wayland() {
        assert!(!is_wayland_session_from_env(Some("x11"), None, Some(":0")));
    }
}
