use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::config::SourceEntry;
use crate::paths::expand_home;

const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "avif", "bmp", "gif"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceImage {
    pub path: PathBuf,
    pub source_id: String,
}

pub fn list_images(entry: &SourceEntry) -> anyhow::Result<Vec<SourceImage>> {
    let path = resolve_path(entry)?;
    collect_from_path(&path)
}

pub fn list_images_with_paths(
    entry: &SourceEntry,
    favorites: &Path,
    fetched: &Path,
) -> anyhow::Result<Vec<SourceImage>> {
    let path = match entry.source_type.as_str() {
        "favorites" => favorites.to_path_buf(),
        "fetched" => fetched.to_path_buf(),
        _ => return list_images(entry),
    };
    let mut e = entry.clone();
    e.source_type = "folder".into();
    e.path = Some(path.display().to_string());
    list_images(&e)
}

fn resolve_path(entry: &SourceEntry) -> anyhow::Result<PathBuf> {
    match entry.source_type.as_str() {
        "folder" | "image" => {
            let p = entry
                .path
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("missing path"))?;
            Ok(expand_home(p))
        }
        "favorites" | "fetched" => {
            anyhow::bail!("favorites/fetched need WallsPaths — use list_images_with_paths")
        }
        other => anyhow::bail!("unsupported source type: {other}"),
    }
}

fn collect_from_path(path: &Path) -> anyhow::Result<Vec<SourceImage>> {
    if path.is_file() {
        if is_image(path) {
            return Ok(vec![make_source_image(path)]);
        }
        return Ok(vec![]);
    }
    if !path.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for dent in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if !dent.file_type().is_file() {
            continue;
        }
        let p = dent.path();
        if is_image(p) {
            out.push(make_source_image(p));
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn make_source_image(p: &Path) -> SourceImage {
    SourceImage {
        path: p.to_path_buf(),
        source_id: p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    }
}

fn is_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXT.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}