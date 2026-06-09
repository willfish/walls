pub fn missing_current_wallpaper() -> &'static str {
    "no current wallpaper. Run `walls apply <path>` or `walls next --manual` first."
}

pub fn next_no_change() -> &'static str {
    "no change. Run `walls next --manual --verbose` to see provider skips, or `walls doctor` to check source readiness."
}

pub fn no_previous_wallpaper() -> &'static str {
    "no previous wallpaper. Apply or advance at least two wallpapers before using previous."
}

pub fn missing_previous_wallpaper(path: &std::path::Path) -> String {
    format!(
        "previous wallpaper file is missing: {}. Re-apply an available wallpaper with `walls apply <path>`, or use `walls current --json` to inspect the current state.",
        path.display()
    )
}

pub fn missing_apply_original(path: &std::path::Path) -> String {
    format!(
        "wallpaper file does not exist: {}. Choose an existing image path, or run `walls next --manual --verbose` to select from configured sources.",
        path.display()
    )
}

pub fn fetch_requires_path() -> &'static str {
    "fetch requires at least one image path. Run `walls fetch <path>...` or use `walls next --manual --verbose` to select from configured sources."
}

pub fn tray_skip_with_recovery(reason: &str) -> String {
    format!("{reason}; {}", tray_skip_recovery_hint(reason))
}

fn tray_skip_recovery_hint(reason: &str) -> &'static str {
    if reason.starts_with("tray disabled (WALLS_TRAY=0)") {
        "unset WALLS_TRAY or set WALLS_TRAY=1 to force tray startup"
    } else if reason.starts_with("no graphical session") {
        "start walls from a Wayland/X11 desktop session, or set WALLS_TRAY=0 when running headless"
    } else if reason.contains("not supported for walls-tray") {
        "use `walls tui` for rotation controls on this desktop, or set WALLS_TRAY=1 to force a tray attempt"
    } else if reason.starts_with("tray autostart disabled") {
        "enable a desktop under tray.autostart.desktops, then run `walls config sync --dry-run`"
    } else if reason.starts_with("walls-tray not found at") {
        "install/build walls-tray or set WALLS_TRAY_BIN=/path/to/walls-tray"
    } else {
        "run `walls doctor` to inspect tray readiness"
    }
}

pub fn tui_next_no_change() -> String {
    format!("next: {}", next_no_change())
}

pub fn tui_no_previous() -> String {
    format!("prev: {}", no_previous_wallpaper())
}

pub fn next_error(error: &anyhow::Error) -> String {
    current_required_error("next", error)
}

pub fn prev_error(error: &walls_core::error::WallsError) -> String {
    match error {
        walls_core::error::WallsError::PreviousOriginalMissing { path } => {
            format!("prev error: {}", missing_previous_wallpaper(path))
        }
        _ => format!("prev error: {error}"),
    }
}

pub fn favorite_error(error: &anyhow::Error) -> String {
    current_required_error("favorite", error)
}

pub fn current_required_error(command: &str, error: &anyhow::Error) -> String {
    let message = error.to_string();
    if is_missing_current_error(error) {
        format!("{command} error: {}", missing_current_wallpaper())
    } else {
        format!("{command} error: {message}")
    }
}

pub fn is_missing_current_error(error: &anyhow::Error) -> bool {
    error.to_string().contains("no current wallpaper")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_messages_include_concrete_next_actions() {
        assert!(missing_current_wallpaper().contains("walls apply <path>"));
        assert!(next_no_change().contains("walls next --manual --verbose"));
        assert!(next_no_change().contains("walls doctor"));
        assert!(no_previous_wallpaper().contains("at least two wallpapers"));
        assert!(
            missing_previous_wallpaper(std::path::Path::new("/tmp/missing.jpg"))
                .contains("walls apply <path>")
        );
        assert!(
            missing_apply_original(std::path::Path::new("/tmp/missing.jpg"))
                .contains("walls next --manual --verbose")
        );
        assert!(fetch_requires_path().contains("walls fetch <path>"));
        assert!(
            tray_skip_with_recovery("tray disabled (WALLS_TRAY=0)").contains("unset WALLS_TRAY")
        );
        assert!(
            tray_skip_with_recovery("no graphical session (tray needs Wayland or X11)")
                .contains("Wayland/X11")
        );
        assert!(tray_skip_with_recovery("tray autostart disabled")
            .contains("walls config sync --dry-run"));
        assert!(
            tray_skip_with_recovery("walls-tray not found at /tmp/walls-tray")
                .contains("WALLS_TRAY_BIN")
        );
    }

    #[test]
    fn tui_messages_keep_existing_prefixes() {
        assert!(tui_next_no_change().starts_with("next: no change"));
        assert!(tui_no_previous().starts_with("prev: no previous"));
    }

    #[test]
    fn favorite_error_rewrites_missing_current_with_recovery() {
        let err = anyhow::anyhow!("no current wallpaper");

        assert_eq!(
            favorite_error(&err),
            "favorite error: no current wallpaper. Run `walls apply <path>` or `walls next --manual` first."
        );
    }

    #[test]
    fn current_required_error_uses_command_prefix() {
        let err = anyhow::anyhow!("no current wallpaper");

        assert_eq!(
            current_required_error("trash", &err),
            "trash error: no current wallpaper. Run `walls apply <path>` or `walls next --manual` first."
        );
    }

    #[test]
    fn prev_error_rewrites_missing_history_file_with_recovery() {
        let err = walls_core::error::WallsError::PreviousOriginalMissing {
            path: std::path::PathBuf::from("/tmp/missing.jpg"),
        };

        let message = prev_error(&err);

        assert!(message.starts_with("prev error: previous wallpaper file is missing"));
        assert!(message.contains("/tmp/missing.jpg"));
        assert!(message.contains("walls apply <path>"));
    }
}
