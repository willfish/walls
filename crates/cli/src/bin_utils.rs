use std::path::{Path, PathBuf};

/// Resolve the `walls-tray` binary for auto-starting the tray from the TUI/CLI.
/// This is the symmetric counterpart to the resolver in the tray crate.
#[allow(dead_code)]
pub fn resolve_tray_bin() -> PathBuf {
    resolve_tray_bin_from(
        std::env::var("WALLS_TRAY_BIN").ok().as_deref(),
        std::env::current_exe().ok().as_deref(),
    )
}

/// Testable resolver for `walls-tray` next to the walls binary or from `WALLS_TRAY_BIN`.
#[allow(dead_code)]
pub fn resolve_tray_bin_from(_tray_bin_env: Option<&str>, _current_exe: Option<&Path>) -> PathBuf {
    unimplemented!("resolver not implemented")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tray_bin_env_takes_precedence() {
        let path = resolve_tray_bin_from(Some("/custom/walls-tray"), None);
        assert_eq!(path, PathBuf::from("/custom/walls-tray"));
    }

    #[test]
    fn uses_sibling_tray_next_to_current_walls_exe() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("walls");
        fs::write(&exe, b"").unwrap();
        let tray = dir.path().join("walls-tray");
        fs::write(&tray, b"").unwrap();

        let path = resolve_tray_bin_from(None, Some(&exe));
        assert_eq!(path, tray);
    }

    #[test]
    fn falls_back_when_exe_has_no_parent() {
        let path = resolve_tray_bin_from(None, Some(Path::new("walls")));
        assert_eq!(path, PathBuf::from("walls-tray"));
    }

    #[test]
    fn falls_back_when_sibling_missing() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("walls");
        fs::write(&exe, b"").unwrap();

        let path = resolve_tray_bin_from(None, Some(&exe));
        assert_eq!(path, PathBuf::from("walls-tray"));
    }
}
