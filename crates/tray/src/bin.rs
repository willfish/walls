use std::path::{Path, PathBuf};

/// Resolve the `walls` CLI binary for tray menu actions.
pub fn resolve_walls_bin() -> PathBuf {
    resolve_walls_bin_from(
        std::env::var("WALLS_BIN").ok().as_deref(),
        std::env::current_exe().ok().as_deref(),
    )
}

/// Testable resolver for `walls` next to the tray binary or from `WALLS_BIN`.
pub fn resolve_walls_bin_from(walls_bin_env: Option<&str>, current_exe: Option<&Path>) -> PathBuf {
    if let Some(path) = walls_bin_env {
        return PathBuf::from(path);
    }
    if let Some(exe) = current_exe {
        if let Some(parent) = exe.parent() {
            let sibling = parent.join("walls");
            if sibling.is_file() {
                return sibling;
            }
        }
    }
    PathBuf::from("walls")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn walls_bin_env_takes_precedence() {
        let path = resolve_walls_bin_from(Some("/custom/walls"), None);
        assert_eq!(path, PathBuf::from("/custom/walls"));
    }

    #[test]
    fn uses_sibling_walls_next_to_current_exe() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("walls-tray");
        fs::write(&exe, b"").unwrap();
        let walls = dir.path().join("walls");
        fs::write(&walls, b"").unwrap();

        let path = resolve_walls_bin_from(None, Some(&exe));
        assert_eq!(path, walls);
    }

    #[test]
    fn falls_back_when_exe_has_no_parent() {
        let path = resolve_walls_bin_from(None, Some(Path::new("walls")));
        assert_eq!(path, PathBuf::from("walls"));
    }

    #[test]
    fn falls_back_when_sibling_missing() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("walls-tray");
        fs::write(&exe, b"").unwrap();

        let path = resolve_walls_bin_from(None, Some(&exe));
        assert_eq!(path, PathBuf::from("walls"));
    }
}
