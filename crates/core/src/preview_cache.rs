use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use image::ImageReader;

const PREVIEW_DIR: &str = "previews";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreviewSize {
    pub width: u32,
    pub height: u32,
}

impl PreviewSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
        }
    }
}

pub fn preview_thumbnail_path(
    source: &Path,
    cache_dir: &Path,
    size: PreviewSize,
) -> anyhow::Result<PathBuf> {
    let metadata = fs::metadata(source)?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok());

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    modified
        .map(|duration| duration.as_nanos())
        .hash(&mut hasher);
    size.hash(&mut hasher);

    Ok(cache_dir
        .join(PREVIEW_DIR)
        .join(format!("{:016x}.png", hasher.finish())))
}

pub fn ensure_preview_thumbnail(
    source: &Path,
    cache_dir: &Path,
    size: PreviewSize,
) -> anyhow::Result<PathBuf> {
    let thumbnail = preview_thumbnail_path(source, cache_dir, size)?;
    if thumbnail.is_file() {
        return Ok(thumbnail);
    }

    if let Some(parent) = thumbnail.parent() {
        fs::create_dir_all(parent)?;
    }

    let image = ImageReader::open(source)?.decode()?;
    let image = image.thumbnail(size.width, size.height);
    let tmp = thumbnail.with_extension(format!("png.tmp-{}", std::process::id()));
    let result = image.save_with_format(&tmp, image::ImageFormat::Png);
    if let Err(error) = result {
        let _ = fs::remove_file(&tmp);
        return Err(error.into());
    }
    fs::rename(&tmp, &thumbnail)?;
    Ok(thumbnail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn preview_thumbnail_is_reused_for_unchanged_source_and_size() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let source = tmp.path().join("wall.png");
        let image = ImageBuffer::from_pixel(100, 80, Rgba([10u8, 20, 30, 255]));
        image.save(&source).expect("source image");

        let cache = tmp.path().join("cache");
        let size = PreviewSize::new(40, 20);
        let first = ensure_preview_thumbnail(&source, &cache, size).expect("first thumbnail");
        let second = ensure_preview_thumbnail(&source, &cache, size).expect("second thumbnail");

        assert_eq!(first, second);
        assert!(first.is_file());
        let thumb = image::open(first).expect("thumbnail");
        assert!(thumb.width() <= 40);
        assert!(thumb.height() <= 20);
    }
}
