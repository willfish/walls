use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::WallsPaths;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NukeDownloadsMode {
    ClearQueue,
    PurgeProviderFiles,
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NukeDownloadsPlan {
    pub mode: NukeDownloadsMode,
    pub queue_len: usize,
    pub cache_files: usize,
    pub download_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NukeDownloadsResult {
    pub mode: NukeDownloadsMode,
    pub queue_cleared: usize,
    pub cache_removed: usize,
    pub download_removed: usize,
}

/// Returns true when `name` is a provider-fetched artifact stored under `cache_dir`.
pub fn is_provider_cache_file_name(name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    stem.starts_with("wallhaven-")
        || stem.starts_with("unsplash-")
        || matches!(stem, "bing-daily" | "json-feed" | "mediarss")
}

pub fn plan_nuke_downloads(paths: &WallsPaths, state: &State) -> NukeDownloadsPlan {
    if !state.cache_queue.is_empty() {
        return NukeDownloadsPlan {
            mode: NukeDownloadsMode::ClearQueue,
            queue_len: state.cache_queue.len(),
            cache_files: 0,
            download_files: 0,
        };
    }

    let cache_files = count_provider_cache_files(&paths.cache_dir);
    let download_files = count_dir_files(&paths.download_dir);
    let mode = if cache_files == 0 && download_files == 0 {
        NukeDownloadsMode::Nothing
    } else {
        NukeDownloadsMode::PurgeProviderFiles
    };

    NukeDownloadsPlan {
        mode,
        queue_len: 0,
        cache_files,
        download_files,
    }
}

pub fn nuke_downloads(
    paths: &WallsPaths,
    state: &mut State,
) -> anyhow::Result<NukeDownloadsResult> {
    let plan = plan_nuke_downloads(paths, state);
    match plan.mode {
        NukeDownloadsMode::ClearQueue => {
            let cleared = state.cache_queue.len();
            state.cache_queue.clear();
            Ok(NukeDownloadsResult {
                mode: NukeDownloadsMode::ClearQueue,
                queue_cleared: cleared,
                cache_removed: 0,
                download_removed: 0,
            })
        }
        NukeDownloadsMode::PurgeProviderFiles => {
            let cache_removed = remove_provider_cache_files(&paths.cache_dir);
            let download_removed = remove_all_dir_files(&paths.download_dir);
            state.wallhaven.collection_pages.clear();
            state.wallhaven.search_page = 0;
            prune_state_after_provider_purge(paths, state);
            Ok(NukeDownloadsResult {
                mode: NukeDownloadsMode::PurgeProviderFiles,
                queue_cleared: 0,
                cache_removed,
                download_removed,
            })
        }
        NukeDownloadsMode::Nothing => Ok(NukeDownloadsResult {
            mode: NukeDownloadsMode::Nothing,
            queue_cleared: 0,
            cache_removed: 0,
            download_removed: 0,
        }),
    }
}

fn count_provider_cache_files(cache_dir: &Path) -> usize {
    read_dir_files(cache_dir)
        .into_iter()
        .filter(|name| is_provider_cache_file_name(name))
        .count()
}

fn count_dir_files(dir: &Path) -> usize {
    read_dir_files(dir).len()
}

fn read_dir_files(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if entry.file_type().ok()?.is_file() {
                entry.file_name().into_string().ok()
            } else {
                None
            }
        })
        .collect()
}

fn remove_provider_cache_files(cache_dir: &Path) -> usize {
    let mut removed = 0usize;
    for name in read_dir_files(cache_dir) {
        if !is_provider_cache_file_name(&name) {
            continue;
        }
        if fs::remove_file(cache_dir.join(&name)).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn remove_all_dir_files(dir: &Path) -> usize {
    let mut removed = 0usize;
    for name in read_dir_files(dir) {
        if fs::remove_file(dir.join(&name)).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn prune_state_after_provider_purge(paths: &WallsPaths, state: &mut State) {
    state.history.retain(|entry| {
        let path = PathBuf::from(entry);
        path.exists() && !is_under_provider_storage(paths, &path)
    });
    if state.history_index >= state.history.len() && !state.history.is_empty() {
        state.history_index = state.history.len() - 1;
    }

    if let Some(current) = &state.current {
        let original = PathBuf::from(&current.original_path);
        let composed = PathBuf::from(&current.composed_path);
        let original_gone = !original.exists() || is_under_provider_storage(paths, &original);
        let composed_gone = composed != original
            && (!composed.exists() || is_under_provider_storage(paths, &composed));
        if original_gone || composed_gone {
            state.current = None;
        }
    }
}

fn is_under_provider_storage(paths: &WallsPaths, path: &Path) -> bool {
    path.starts_with(&paths.cache_dir) || path.starts_with(&paths.download_dir)
}
