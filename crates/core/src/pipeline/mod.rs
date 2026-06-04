use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};

use crate::config::DisplayConfig;
use crate::paths::WallsPaths;

/// Compose the final wallpaper image (filters, clock, quotes — stubs for now).
pub fn compose(
    paths: &WallsPaths,
    display: &DisplayConfig,
    original: &Path,
) -> anyhow::Result<PathBuf> {
    if !original.exists() {
        anyhow::bail!("wallpaper file does not exist: {}", original.display());
    }

    if display.auto_rotate {
        if let Some(rotated) = auto_rotate(paths, original)? {
            return Ok(rotated);
        }
    }

    Ok(original.to_path_buf())
}

fn auto_rotate(paths: &WallsPaths, original: &Path) -> anyhow::Result<Option<PathBuf>> {
    let Ok(reader) = ImageReader::open(original).and_then(|r| r.with_guessed_format()) else {
        return Ok(None);
    };
    let Ok(mut decoder) = reader.into_decoder() else {
        return Ok(None);
    };
    let Ok(orientation) = decoder.orientation() else {
        return Ok(None);
    };
    if orientation == image::metadata::Orientation::NoTransforms {
        return Ok(None);
    }

    std::fs::create_dir_all(&paths.compose_dir)?;
    let Ok(mut image) = DynamicImage::from_decoder(decoder) else {
        return Ok(None);
    };
    image.apply_orientation(orientation);

    let output = auto_rotated_path(paths, original);
    image.save_with_format(&output, ImageFormat::Png)?;
    Ok(Some(output))
}

fn auto_rotated_path(paths: &WallsPaths, original: &Path) -> PathBuf {
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("wallpaper");
    paths
        .compose_dir
        .join(format!("{stem}.{}.auto-rotated.png", fnv1a64(original)))
}

fn fnv1a64(path: &Path) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
