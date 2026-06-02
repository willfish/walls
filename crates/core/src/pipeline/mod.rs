use std::path::{Path, PathBuf};

use crate::paths::WallsPaths;

/// Compose the final wallpaper image (filters, clock, quotes — stubs for now).
pub fn compose(paths: &WallsPaths, original: &Path) -> anyhow::Result<PathBuf> {
    // M1: pass-through — use original path directly.
    if !original.exists() {
        anyhow::bail!("wallpaper file does not exist: {}", original.display());
    }
    let _ = paths.compose_dir.as_path();
    Ok(original.to_path_buf())
}
