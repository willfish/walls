use std::path::{Path, PathBuf};
use std::process::Command;

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use rand::seq::IndexedRandom;

use crate::config::{DisplayConfig, ImageMagickFilterConfig};
use crate::paths::WallsPaths;

/// Compose the final wallpaper image.
pub fn compose(
    paths: &WallsPaths,
    display: &DisplayConfig,
    original: &Path,
) -> anyhow::Result<PathBuf> {
    if !original.exists() {
        anyhow::bail!("wallpaper file does not exist: {}", original.display());
    }

    let mut current = original.to_path_buf();

    if display.auto_rotate {
        if let Some(rotated) = auto_rotate(paths, original)? {
            current = rotated;
        }
    }

    if display.filters.enabled {
        if let Some(filtered) = apply_random_filter(paths, display, &current)? {
            current = filtered;
        }
    }

    Ok(current)
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

fn apply_random_filter(
    paths: &WallsPaths,
    display: &DisplayConfig,
    input: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(filter) = display.filters.filters.choose(&mut rand::rng()) else {
        return Ok(None);
    };

    std::fs::create_dir_all(&paths.compose_dir)?;
    let output = filtered_path(paths, input, filter);
    run_imagemagick_filter(&display.filters.command, input, filter, &output)?;
    Ok(Some(output))
}

fn run_imagemagick_filter(
    command: &str,
    input: &Path,
    filter: &ImageMagickFilterConfig,
    output: &Path,
) -> anyhow::Result<()> {
    let status = Command::new(command)
        .arg(input)
        .args(&filter.args)
        .arg(output)
        .status()?;
    if !status.success() {
        anyhow::bail!(
            "ImageMagick filter '{}' failed for {}: {status}",
            filter.name,
            input.display()
        );
    }
    Ok(())
}

fn filtered_path(paths: &WallsPaths, input: &Path, filter: &ImageMagickFilterConfig) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("wallpaper");
    paths.compose_dir.join(format!(
        "{stem}.{}.{}.png",
        fnv1a64(input),
        filter_name_slug(&filter.name)
    ))
}

fn filter_name_slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "filter".into()
    } else {
        slug.into()
    }
}

fn fnv1a64(path: &Path) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
