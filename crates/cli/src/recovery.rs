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

pub fn tui_next_no_change() -> String {
    format!("next: {}", next_no_change())
}

pub fn tui_no_previous() -> String {
    format!("prev: {}", no_previous_wallpaper())
}

pub fn favorite_error(error: &anyhow::Error) -> String {
    let message = error.to_string();
    if message.contains("no current wallpaper") {
        format!("favorite error: {}", missing_current_wallpaper())
    } else {
        format!("favorite error: {error}")
    }
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
}
