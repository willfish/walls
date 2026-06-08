//! Linux tray host detection (StatusNotifier vs legacy AppIndicator).

use walls_core::tray::session_type_from_env;

/// Whether the session is Wayland (SNI is the native tray path).
pub fn is_wayland_session() -> bool {
    matches!(
        session_type_from_env(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
            std::env::var("DISPLAY").ok().as_deref(),
        ),
        walls_core::tray::SessionType::Wayland
    )
}

/// Prefer StatusNotifierItem on Wayland; AppIndicator remains the X11 path.
pub fn prefer_status_notifier() -> bool {
    is_wayland_session()
}
