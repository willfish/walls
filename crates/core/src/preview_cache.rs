use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use image::ImageReader;

use crate::state::State;

const PREVIEW_DIR: &str = "previews";
pub const DEFAULT_PREVIEW_SIZE: PreviewSize = PreviewSize {
    width: 1024,
    height: 1024,
};

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

pub fn previewable_paths_from_state(state: &State, cache_dir: &Path) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();

    for id in &state.cache_queue {
        let path = if let Some(photo_id) = crate::unsplash::queue_photo_id(id) {
            crate::unsplash::cached_photo_path(cache_dir, photo_id)
        } else {
            crate::wallhaven::cached_wallpaper_path(cache_dir, id)
        };
        if let Some(path) = path {
            push_previewable_path(&mut paths, &mut seen, path);
        }
    }

    for entry in &state.history {
        let path = PathBuf::from(entry);
        if path.is_file() {
            push_previewable_path(&mut paths, &mut seen, path);
        }
    }

    paths
}

pub fn prewarm_preview_thumbnails(
    sources: impl IntoIterator<Item = PathBuf>,
    cache_dir: &Path,
    size: PreviewSize,
) -> PreviewPrewarmStats {
    let mut stats = PreviewPrewarmStats::default();
    let mut seen = HashSet::new();
    for source in sources {
        if !seen.insert(source.clone()) {
            continue;
        }
        stats.attempted += 1;
        match ensure_preview_thumbnail(&source, cache_dir, size) {
            Ok(_) => stats.warmed += 1,
            Err(err) => {
                stats.failed += 1;
                tracing::debug!("preview prewarm failed for {}: {err:#}", source.display());
            }
        }
    }
    stats
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PreviewPrewarmStats {
    pub attempted: usize,
    pub warmed: usize,
    pub failed: usize,
}

fn push_previewable_path(paths: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if seen.insert(path.clone()) {
        paths.push(path);
    }
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

    #[test]
    fn previewable_paths_include_cached_queue_then_history_without_duplicates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cache = tmp.path().join("cache");
        fs::create_dir_all(&cache).expect("cache dir");
        let wallhaven = cache.join("wallhaven-abc123.jpg");
        let unsplash = cache.join("unsplash-photo1.png");
        let history = tmp.path().join("history.jpg");
        fs::write(&wallhaven, b"wallhaven").expect("wallhaven");
        fs::write(&unsplash, b"unsplash").expect("unsplash");
        fs::write(&history, b"history").expect("history");

        let state = State {
            cache_queue: vec!["abc123".into(), "unsplash:photo1".into(), "missing".into()],
            history: vec![
                history.display().to_string(),
                wallhaven.display().to_string(),
                history.display().to_string(),
                tmp.path().join("missing.jpg").display().to_string(),
            ],
            ..State::default()
        };

        assert_eq!(
            previewable_paths_from_state(&state, &cache),
            vec![wallhaven, unsplash, history]
        );
    }

    #[test]
    fn prewarm_preview_thumbnails_deduplicates_sources_and_counts_failures() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let source = tmp.path().join("wall.png");
        let image = ImageBuffer::from_pixel(100, 80, Rgba([10u8, 20, 30, 255]));
        image.save(&source).expect("source image");
        let missing = tmp.path().join("missing.png");
        let cache = tmp.path().join("cache");

        let stats = prewarm_preview_thumbnails(
            vec![source.clone(), source.clone(), missing],
            &cache,
            PreviewSize::new(40, 40),
        );

        assert_eq!(
            stats,
            PreviewPrewarmStats {
                attempted: 2,
                warmed: 1,
                failed: 1,
            }
        );
        assert!(
            preview_thumbnail_path(&source, &cache, PreviewSize::new(40, 40))
                .expect("thumbnail path")
                .is_file()
        );
    }
}
