pub fn missing_current_wallpaper() -> &'static str {
    "no current wallpaper. Run `walls apply <path>` or `walls next --manual` first."
}

pub fn next_no_change() -> &'static str {
    "no change. Run `walls next --manual --verbose` to see provider skips, or `walls doctor` to check source readiness."
}

pub fn no_previous_wallpaper() -> &'static str {
    "no previous wallpaper. Apply or advance at least two wallpapers before using previous."
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
