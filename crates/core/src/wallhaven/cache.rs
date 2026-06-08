use std::path::{Path, PathBuf};

/// Extensions used when Wallhaven downloads are stored under `cache_dir`.
const CACHE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    #[test]
    fn finds_standard_jpg_without_directory_scan() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallhaven-abc123.jpg");
        fs::write(&path, b"x").unwrap();
        fs::write(dir.path().join("other-file.png"), b"y").unwrap();

        assert_eq!(
            cached_wallpaper_path(dir.path(), "abc123").as_deref(),
            Some(path.as_path())
        );
    }

    #[test]
    fn finds_webp_and_png_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let webp = dir.path().join("wallhaven-xyz.webp");
        fs::write(&webp, b"x").unwrap();
        assert_eq!(
            cached_wallpaper_path(dir.path(), "xyz").as_deref(),
            Some(webp.as_path())
        );

        fs::remove_file(&webp).unwrap();
        let png = dir.path().join("wallhaven-xyz.png");
        fs::write(&png, b"x").unwrap();
        assert_eq!(
            cached_wallpaper_path(dir.path(), "xyz").as_deref(),
            Some(png.as_path())
        );
    }

    #[test]
    fn legacy_extension_falls_back_to_stem_scan() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallhaven-legacy.gif");
        fs::write(&path, b"x").unwrap();

        assert_eq!(
            cached_wallpaper_path(dir.path(), "legacy").as_deref(),
            Some(path.as_path())
        );
    }

    #[test]
    fn missing_id_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(cached_wallpaper_path(dir.path(), "nope").is_none());
        assert!(cached_wallpaper_path(Path::new("/nonexistent-dir-xyz"), "id").is_none());
    }
}
