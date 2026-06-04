use std::fs;
use std::path::Path;

use walls_core::wallhaven::cached_wallpaper_path;

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
