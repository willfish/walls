mod local;

pub use local::{list_images, list_images_with_paths, SourceImage};

use crate::config::SourceEntry;

pub fn enabled_sources(entries: &[SourceEntry]) -> Vec<&SourceEntry> {
    entries.iter().filter(|s| s.enabled).collect()
}