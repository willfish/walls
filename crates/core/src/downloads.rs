use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::WallsPaths;
use crate::state::State;

static ATOMIC_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NukeDownloadsMode {
    ClearQueue,
    PurgeProviderFiles,
    ProviderReset,
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NukeDownloadsPlan {
    pub mode: NukeDownloadsMode,
    pub queue_len: usize,
    pub cache_files: usize,
    pub download_files: usize,
    pub history_provider_entries: usize,
    pub current_provider_storage: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NukeDownloadsResult {
    pub mode: NukeDownloadsMode,
    pub queue_cleared: usize,
    pub cache_removed: usize,
    pub download_removed: usize,
    pub history_pruned: usize,
    pub current_cleared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheDirStats {
    pub files: usize,
    pub bytes: u64,
    pub provider_files: usize,
    pub provider_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheInspection {
    pub cache: CacheDirStats,
    pub downloads: CacheDirStats,
    pub queue_len: usize,
    pub queue_ids: Vec<String>,
    pub current_provider_storage: bool,
    pub history_provider_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheFileEntry {
    pub area: CacheFileArea,
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheFileArea {
    Cache,
    Downloads,
}

pub async fn write_file_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = atomic_tmp_path(path);
    let result = async {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&tmp, bytes).await?;
        let file = tokio::fs::OpenOptions::new().write(true).open(&tmp).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&tmp, path).await?;
        Ok(())
    }
    .await;

    if result.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
    }
    result
}

pub async fn copy_file_atomic(from: &Path, to: &Path) -> anyhow::Result<()> {
    let tmp = atomic_tmp_path(to);
    let result = async {
        if let Some(parent) = to.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(from, &tmp).await?;
        let file = tokio::fs::OpenOptions::new().write(true).open(&tmp).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&tmp, to).await?;
        Ok(())
    }
    .await;

    if result.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
    }
    result
}

/// Returns true when `name` is a provider-fetched artifact stored under `cache_dir`.
pub fn is_provider_cache_file_name(name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    stem.starts_with("wallhaven-")
        || stem.starts_with("unsplash-")
        || matches!(
            stem,
            "bing-daily"
                | "json-feed"
                | "mediarss"
                | "reddit-fetch"
                | "apod-daily"
                | "pixabay-fetch"
                | "immich-fetch"
                | "attribution-fetch"
        )
}

pub fn plan_nuke_downloads(paths: &WallsPaths, state: &State) -> NukeDownloadsPlan {
    let queue_len = state.cache_queue.len();
    let cache_files = count_dir_files(&paths.cache_dir);
    let download_files = count_dir_files(&paths.download_dir);
    let history_provider_entries = count_history_provider_entries(paths, state);
    let current_provider_storage = current_uses_provider_storage(paths, state);
    let mode = if queue_len == 0
        && cache_files == 0
        && download_files == 0
        && history_provider_entries == 0
        && !current_provider_storage
    {
        NukeDownloadsMode::Nothing
    } else {
        NukeDownloadsMode::ProviderReset
    };

    NukeDownloadsPlan {
        mode,
        queue_len,
        cache_files,
        download_files,
        history_provider_entries,
        current_provider_storage,
    }
}

pub fn inspect_cache(paths: &WallsPaths, state: &State) -> CacheInspection {
    CacheInspection {
        cache: dir_stats(&paths.cache_dir, true),
        downloads: dir_stats(&paths.download_dir, false),
        queue_len: state.cache_queue.len(),
        queue_ids: state.cache_queue.clone(),
        current_provider_storage: current_uses_provider_storage(paths, state),
        history_provider_entries: count_history_provider_entries(paths, state),
    }
}

pub fn list_cache_files(paths: &WallsPaths, provider: Option<&str>) -> Vec<CacheFileEntry> {
    let mut entries = Vec::new();
    entries.extend(list_area_files(
        CacheFileArea::Cache,
        &paths.cache_dir,
        true,
        provider,
    ));
    entries.extend(list_area_files(
        CacheFileArea::Downloads,
        &paths.download_dir,
        false,
        provider,
    ));
    entries.sort_by(|left, right| {
        left.area
            .label()
            .cmp(right.area.label())
            .then_with(|| left.name.cmp(&right.name))
    });
    entries
}

pub fn clear_cache_queue(state: &mut State) -> usize {
    let cleared = state.cache_queue.len();
    state.cache_queue.clear();
    cleared
}

pub fn purge_provider_files(paths: &WallsPaths, state: &mut State) -> NukeDownloadsResult {
    let cache_removed = remove_provider_cache_files(&paths.cache_dir);
    let download_removed = remove_all_dir_files(&paths.download_dir);
    state.wallhaven.collection_pages.clear();
    state.wallhaven.search_page = 0;
    let state_prune = prune_state_after_provider_purge(paths, state);
    NukeDownloadsResult {
        mode: NukeDownloadsMode::PurgeProviderFiles,
        queue_cleared: 0,
        cache_removed,
        download_removed,
        history_pruned: state_prune.history_pruned,
        current_cleared: state_prune.current_cleared,
    }
}

pub fn reset_provider_storage(paths: &WallsPaths, state: &mut State) -> NukeDownloadsResult {
    let queue_cleared = clear_cache_queue(state);
    let cache_removed = remove_all_dir_files(&paths.cache_dir);
    let download_removed = remove_all_dir_files(&paths.download_dir);
    state.wallhaven.collection_pages.clear();
    state.wallhaven.search_page = 0;
    let state_prune = prune_state_after_provider_purge(paths, state);
    NukeDownloadsResult {
        mode: NukeDownloadsMode::ProviderReset,
        queue_cleared,
        cache_removed,
        download_removed,
        history_pruned: state_prune.history_pruned,
        current_cleared: state_prune.current_cleared,
    }
}

pub fn nuke_downloads(
    paths: &WallsPaths,
    state: &mut State,
) -> anyhow::Result<NukeDownloadsResult> {
    let plan = plan_nuke_downloads(paths, state);
    match plan.mode {
        NukeDownloadsMode::ClearQueue => {
            let cleared = clear_cache_queue(state);
            Ok(NukeDownloadsResult {
                mode: NukeDownloadsMode::ClearQueue,
                queue_cleared: cleared,
                cache_removed: 0,
                download_removed: 0,
                history_pruned: 0,
                current_cleared: false,
            })
        }
        NukeDownloadsMode::PurgeProviderFiles => Ok(purge_provider_files(paths, state)),
        NukeDownloadsMode::ProviderReset => Ok(reset_provider_storage(paths, state)),
        NukeDownloadsMode::Nothing => Ok(NukeDownloadsResult {
            mode: NukeDownloadsMode::Nothing,
            queue_cleared: 0,
            cache_removed: 0,
            download_removed: 0,
            history_pruned: 0,
            current_cleared: false,
        }),
    }
}

impl CacheFileArea {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cache => "cache",
            Self::Downloads => "downloads",
        }
    }
}

impl NukeDownloadsMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::ClearQueue => "clear_queue",
            Self::PurgeProviderFiles => "purge_provider_files",
            Self::ProviderReset => "provider_reset",
            Self::Nothing => "nothing",
        }
    }
}

fn dir_stats(dir: &Path, provider_cache_only: bool) -> CacheDirStats {
    let mut stats = CacheDirStats {
        files: 0,
        bytes: 0,
        provider_files: 0,
        provider_bytes: 0,
    };
    for entry in file_entries(dir) {
        stats.files += 1;
        stats.bytes = stats.bytes.saturating_add(entry.bytes);
        let provider_file = if provider_cache_only {
            is_provider_cache_file_name(&entry.name)
        } else {
            true
        };
        if provider_file {
            stats.provider_files += 1;
            stats.provider_bytes = stats.provider_bytes.saturating_add(entry.bytes);
        }
    }
    stats
}

fn list_area_files(
    area: CacheFileArea,
    dir: &Path,
    provider_cache_only: bool,
    provider: Option<&str>,
) -> Vec<CacheFileEntry> {
    file_entries(dir)
        .into_iter()
        .filter_map(|entry| {
            let provider_name = if provider_cache_only {
                provider_name_from_cache_file(&entry.name)
            } else {
                provider_name_from_download_file(&entry.name)
            };
            if provider_cache_only && provider_name.is_none() {
                return None;
            }
            if provider.is_some_and(|filter| provider_name.as_deref() != Some(filter)) {
                return None;
            }
            Some(CacheFileEntry {
                area,
                name: entry.name,
                path: entry.path,
                bytes: entry.bytes,
                provider: provider_name,
            })
        })
        .collect()
}

struct FsFileEntry {
    name: String,
    path: PathBuf,
    bytes: u64,
}

fn file_entries(dir: &Path) -> Vec<FsFileEntry> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if !entry.file_type().ok()?.is_file() {
                return None;
            }
            let name = entry.file_name().into_string().ok()?;
            let bytes = entry.metadata().ok()?.len();
            Some(FsFileEntry {
                name,
                path: entry.path(),
                bytes,
            })
        })
        .collect()
}

fn provider_name_from_cache_file(name: &str) -> Option<String> {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    if stem.starts_with("wallhaven-") {
        Some("wallhaven".into())
    } else if stem.starts_with("unsplash-") {
        Some("unsplash".into())
    } else {
        match stem {
            "bing-daily" => Some("bing".into()),
            "json-feed" => Some("json-feed".into()),
            "mediarss" => Some("mediarss".into()),
            "reddit-fetch" => Some("reddit".into()),
            "apod-daily" => Some("apod".into()),
            "pixabay-fetch" => Some("pixabay".into()),
            "immich-fetch" => Some("immich".into()),
            "attribution-fetch" => Some("attribution".into()),
            _ => None,
        }
    }
}

fn provider_name_from_download_file(name: &str) -> Option<String> {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    stem.split_once('-')
        .map(|(provider, _)| provider.to_string())
        .or_else(|| Some("downloaded".into()))
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
    for entry in fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let removed_entry = if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            fs::remove_dir_all(&path).is_ok()
        } else {
            fs::remove_file(&path).is_ok()
        };
        if removed_entry {
            removed = removed.saturating_add(1);
        }
    }
    removed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatePruneResult {
    history_pruned: usize,
    current_cleared: bool,
}

fn prune_state_after_provider_purge(paths: &WallsPaths, state: &mut State) -> StatePruneResult {
    let history_before = state.history.len();
    state.history.retain(|entry| {
        let path = PathBuf::from(entry);
        !is_under_provider_storage(paths, &path)
    });
    if state.history_index >= state.history.len() && !state.history.is_empty() {
        state.history_index = state.history.len() - 1;
    }

    let mut current_cleared = false;
    if let Some(current) = &state.current {
        let original = PathBuf::from(&current.original_path);
        let composed = PathBuf::from(&current.composed_path);
        if is_under_provider_storage(paths, &original)
            || is_under_provider_storage(paths, &composed)
        {
            state.current = None;
            current_cleared = true;
        }
    }

    StatePruneResult {
        history_pruned: history_before.saturating_sub(state.history.len()),
        current_cleared,
    }
}

fn count_history_provider_entries(paths: &WallsPaths, state: &State) -> usize {
    state
        .history
        .iter()
        .filter(|entry| is_under_provider_storage(paths, Path::new(entry)))
        .count()
}

fn current_uses_provider_storage(paths: &WallsPaths, state: &State) -> bool {
    state.current.as_ref().is_some_and(|current| {
        let original = PathBuf::from(&current.original_path);
        let composed = PathBuf::from(&current.composed_path);
        is_under_provider_storage(paths, &original) || is_under_provider_storage(paths, &composed)
    })
}

fn is_under_provider_storage(paths: &WallsPaths, path: &Path) -> bool {
    path.starts_with(&paths.cache_dir) || path.starts_with(&paths.download_dir)
}

fn atomic_tmp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let counter = ATOMIC_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    path.with_file_name(format!(
        ".{file_name}.tmp-{}-{nanos}-{counter}",
        std::process::id()
    ))
}
