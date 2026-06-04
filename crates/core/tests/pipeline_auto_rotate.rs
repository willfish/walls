use std::fs::File;
use std::path::PathBuf;

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, GenericImageView, ImageEncoder};
use tempfile::TempDir;
use walls_core::config::DisplayConfig;
use walls_core::paths::WallsPaths;
use walls_core::pipeline::compose;

#[test]
fn compose_returns_original_when_auto_rotate_disabled() {
    let temp = TempDir::new().unwrap();
    let paths = test_paths(&temp);
    let original = temp.path().join("portrait.jpg");
    write_oriented_jpeg(&original, 6);

    let display = DisplayConfig {
        auto_rotate: false,
        ..Default::default()
    };

    let composed = compose(&paths, &display, &original).unwrap();

    assert_eq!(composed, original);
    assert!(!paths.compose_dir.exists());
}

#[test]
fn compose_returns_original_for_unsupported_auto_rotate_format() {
    let temp = TempDir::new().unwrap();
    let paths = test_paths(&temp);
    let original = temp.path().join("animated.gif");
    std::fs::write(&original, b"not a real gif").unwrap();

    let display = DisplayConfig {
        auto_rotate: true,
        ..Default::default()
    };

    let composed = compose(&paths, &display, &original).unwrap();

    assert_eq!(composed, original);
    assert!(!paths.compose_dir.exists());
}

#[test]
fn compose_writes_oriented_png_when_auto_rotate_enabled() {
    let temp = TempDir::new().unwrap();
    let paths = test_paths(&temp);
    let original = temp.path().join("portrait.jpg");
    write_oriented_jpeg(&original, 6);

    let display = DisplayConfig {
        auto_rotate: true,
        ..Default::default()
    };

    let composed = compose(&paths, &display, &original).unwrap();

    assert_ne!(composed, original);
    assert_eq!(composed.extension().and_then(|s| s.to_str()), Some("png"));

    let image = image::open(composed).unwrap();
    assert_eq!(image.dimensions(), (3, 2));
}

fn test_paths(temp: &TempDir) -> WallsPaths {
    let root = temp.path();
    WallsPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.json"),
        secrets_file: root.join("config/secrets.json"),
        state_file: root.join("state/state.json"),
        cache_dir: root.join("cache"),
        download_dir: root.join("downloaded"),
        favorites_dir: root.join("favorites"),
        fetched_dir: root.join("fetched"),
        compose_dir: root.join("wallpaper"),
    }
}

fn write_oriented_jpeg(path: &PathBuf, orientation: u16) {
    let file = File::create(path).unwrap();
    let mut encoder = JpegEncoder::new_with_quality(file, 100);
    encoder
        .set_exif_metadata(exif_orientation(orientation))
        .unwrap();

    let pixels = [
        255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0, 255, 0, 255, 0, 255, 255,
    ];
    encoder
        .write_image(&pixels, 2, 3, ExtendedColorType::Rgb8)
        .unwrap();
}

fn exif_orientation(orientation: u16) -> Vec<u8> {
    let mut exif = Vec::new();
    exif.extend_from_slice(b"II");
    exif.extend_from_slice(&42_u16.to_le_bytes());
    exif.extend_from_slice(&8_u32.to_le_bytes());
    exif.extend_from_slice(&1_u16.to_le_bytes());
    exif.extend_from_slice(&0x0112_u16.to_le_bytes());
    exif.extend_from_slice(&3_u16.to_le_bytes());
    exif.extend_from_slice(&1_u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&0_u16.to_le_bytes());
    exif.extend_from_slice(&0_u32.to_le_bytes());
    exif
}
