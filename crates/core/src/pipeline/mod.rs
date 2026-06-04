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

    if let Some(display_mode) = apply_display_mode(paths, display, &current)? {
        current = display_mode;
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

fn apply_display_mode(
    paths: &WallsPaths,
    display: &DisplayConfig,
    input: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let Some((width, height)) = display_target_size(display) else {
        return Ok(None);
    };
    let Some(args) = display_mode_args(&display.mode, width, height) else {
        return Ok(None);
    };

    std::fs::create_dir_all(&paths.compose_dir)?;
    let output = display_mode_path(paths, input, &display.mode, width, height);
    run_display_mode_command(&display.imagemagick_command, input, &args, &output)?;
    Ok(Some(output))
}

fn display_target_size(display: &DisplayConfig) -> Option<(u32, u32)> {
    let width = display.target_width?;
    let height = display.target_height?;
    if width == 0 || height == 0 {
        None
    } else {
        Some((width, height))
    }
}

fn display_mode_args(mode: &str, width: u32, height: u32) -> Option<Vec<String>> {
    let geometry = format!("{width}x{height}");
    match mode {
        "zoom" => Some(vec![
            "-resize".into(),
            format!("{geometry}^"),
            "-gravity".into(),
            "center".into(),
            "-extent".into(),
            geometry,
        ]),
        "fill-with-black" => Some(vec![
            "-resize".into(),
            geometry.clone(),
            "-gravity".into(),
            "center".into(),
            "-background".into(),
            "black".into(),
            "-extent".into(),
            geometry,
        ]),
        "fill-with-blur" => Some(vec![
            "(".into(),
            "-clone".into(),
            "0".into(),
            "-resize".into(),
            format!("{geometry}^"),
            "-gravity".into(),
            "center".into(),
            "-extent".into(),
            geometry.clone(),
            "-blur".into(),
            "0x16".into(),
            ")".into(),
            "(".into(),
            "-clone".into(),
            "0".into(),
            "-resize".into(),
            geometry,
            ")".into(),
            "-delete".into(),
            "0".into(),
            "-gravity".into(),
            "center".into(),
            "-composite".into(),
        ]),
        _ => None,
    }
}

fn run_display_mode_command(
    command: &str,
    input: &Path,
    args: &[String],
    output: &Path,
) -> anyhow::Result<()> {
    let status = Command::new(command)
        .arg(input)
        .args(args)
        .arg(output)
        .status()?;
    if !status.success() {
        anyhow::bail!(
            "ImageMagick display mode failed for {}: {status}",
            input.display()
        );
    }
    Ok(())
}

fn display_mode_path(
    paths: &WallsPaths,
    input: &Path,
    mode: &str,
    width: u32,
    height: u32,
) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("wallpaper");
    paths.compose_dir.join(format!(
        "{stem}.{}.{}.{}x{}.png",
        fnv1a64(input),
        filter_name_slug(mode),
        width,
        height
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
