/// Detected desktop environment (mirrors Variety `set_wallpaper` `detect_desktop`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Unity,
    Budgie,
    Kde,
    Xfce,
    Lxde,
    Lxqt,
    Mate,
    Cinnamon,
    Lingmo,
    Deepin,
    Trinity,
    Fluxbox,
    Sway,
    Hyprland,
    Enlightenment,
    Awesome,
    Cosmic,
    Unknown,
}

pub fn detect_desktop() -> Desktop {
    if let Ok(xdg) = std::env::var("XDG_CURRENT_DESKTOP") {
        let lower = xdg.to_lowercase();
        if lower.contains("gnome") {
            return Desktop::Gnome;
        }
        if lower.contains("unity") {
            return Desktop::Unity;
        }
        if lower.contains("budgie") {
            return Desktop::Budgie;
        }
        if lower.contains("kde") {
            return Desktop::Kde;
        }
        if lower.contains("xfce") {
            return Desktop::Xfce;
        }
        if lower.contains("lxde") {
            return Desktop::Lxde;
        }
        if lower.contains("lxqt") {
            return Desktop::Lxqt;
        }
        if lower.contains("mate") {
            return Desktop::Mate;
        }
        if lower.contains("cinnamon") {
            return Desktop::Cinnamon;
        }
        if lower.contains("lingmo") {
            return Desktop::Lingmo;
        }
        if lower.contains("deepin") {
            return Desktop::Deepin;
        }
        if lower.contains("trinity") {
            return Desktop::Trinity;
        }
        if lower.contains("fluxbox") {
            return Desktop::Fluxbox;
        }
        if lower.contains("sway") {
            return Desktop::Sway;
        }
        if lower.contains("hyprland") {
            return Desktop::Hyprland;
        }
        if lower.contains("enlightenment") || lower.contains("moksha") {
            return Desktop::Enlightenment;
        }
        if xdg == "COSMIC" {
            return Desktop::Cosmic;
        }
        return Desktop::Unknown;
    }

    let session = format!(
        "{} {}",
        std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default(),
        std::env::var("DESKTOP_STARTUP_ID").unwrap_or_default()
    );
    if session.to_lowercase().contains("awesome") {
        return Desktop::Awesome;
    }

    Desktop::Unknown
}