//! Resolve companion binaries (`walls` / `walls-tray`) for dev and installed layouts.

use std::path::{Path, PathBuf};

/// Inputs for [`resolve_binary`].
#[derive(Debug, Clone, Copy)]
pub struct BinResolveOpts<'a> {
    /// Override from e.g. `WALLS_BIN` / `WALLS_TRAY_BIN`.
    pub env_var: Option<&'a str>,
    /// `std::env::current_exe()` of the running binary.
    pub current_exe: Option<&'a Path>,
    /// Filename next to `current_exe` (e.g. `walls`, `walls-tray`).
    pub sibling_name: &'a str,
    /// Compile-time default from `build.rs` (`CARGO_TARGET_DIR` + profile).
    pub build_default: Option<&'a str>,
    /// Last resort for `PATH` lookup (e.g. `walls`).
    pub path_fallback: &'a str,
}

/// Resolve a companion binary.
///
/// Precedence: env override → sibling of `current_exe` → compile-time build path → `PATH`.
pub fn resolve_binary(opts: BinResolveOpts<'_>) -> PathBuf {
    if let Some(path) = opts.env_var {
        return PathBuf::from(path);
    }

    if let Some(exe) = opts.current_exe {
        if let Some(parent) = exe.parent() {
            let sibling = parent.join(opts.sibling_name);
            if sibling.is_file() {
                return sibling;
            }
        }
    }

    if let Some(default) = opts.build_default {
        let path = PathBuf::from(default);
        if path.is_file() {
            return path;
        }
    }

    PathBuf::from(opts.path_fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn env_var_takes_precedence() {
        let path = resolve_binary(BinResolveOpts {
            env_var: Some("/custom/walls"),
            current_exe: None,
            sibling_name: "walls",
            build_default: Some("/build/walls"),
            path_fallback: "walls",
        });
        assert_eq!(path, PathBuf::from("/custom/walls"));
    }

    #[test]
    fn uses_sibling_next_to_current_exe() {
        let dir = tempfile::tempdir().unwrap();
        let tray = dir.path().join("walls-tray");
        fs::write(&tray, b"").unwrap();
        let walls = dir.path().join("walls");
        fs::write(&walls, b"").unwrap();

        let path = resolve_binary(BinResolveOpts {
            env_var: None,
            current_exe: Some(&tray),
            sibling_name: "walls",
            build_default: None,
            path_fallback: "walls",
        });
        assert_eq!(path, walls);
    }

    #[test]
    fn uses_build_default_when_sibling_missing() {
        let dir = tempfile::tempdir().unwrap();
        let tray = dir.path().join("walls-tray");
        fs::write(&tray, b"").unwrap();
        let walls = dir.path().join("walls");
        fs::write(&walls, b"").unwrap();

        let path = resolve_binary(BinResolveOpts {
            env_var: None,
            current_exe: Some(&tray),
            sibling_name: "walls",
            build_default: Some(walls.to_str().unwrap()),
            path_fallback: "walls",
        });
        assert_eq!(path, walls);
    }

    #[test]
    fn falls_back_to_path_name() {
        let path = resolve_binary(BinResolveOpts {
            env_var: None,
            current_exe: Some(Path::new("walls-tray")),
            sibling_name: "walls",
            build_default: None,
            path_fallback: "walls",
        });
        assert_eq!(path, PathBuf::from("walls"));
    }
}
