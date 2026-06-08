use std::path::{Path, PathBuf};

const QUEUE_PREFIX: &str = "unsplash:";
const CACHE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

pub(crate) fn queue_id(photo_id: &str) -> String {
    format!("{QUEUE_PREFIX}{photo_id}")
}

pub(crate) fn queue_photo_id(queue_item: &str) -> Option<&str> {
    queue_item.strip_prefix(QUEUE_PREFIX)
}

pub(crate) fn cached_photo_path(cache_dir: &Path, photo_id: &str) -> Option<PathBuf> {
    for ext in CACHE_EXTENSIONS {
        let path = cache_dir.join(format!("unsplash-{photo_id}.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}
