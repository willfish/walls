use std::path::{Path, PathBuf};

/// Extensions used when Wallhaven downloads are stored under `cache_dir`.
pub const CACHE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// Resolve `wallhaven-{id}.{ext}` without scanning the whole cache directory.
pub fn cached_wallpaper_path(cache_dir: &Path, id: &str) -> Option<PathBuf> {
    let stem = format!("wallhaven-{id}");
    for ext in CACHE_EXTENSIONS {
        let path = cache_dir.join(format!("{stem}.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }

    // Legacy or uncommon extensions: single directory pass matching stem only.
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return None;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s == stem)
        {
            return Some(path);
        }
    }
    None
}
