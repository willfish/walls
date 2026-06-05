use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Eq)]
struct QuotaEntry {
    modified: SystemTime,
    ordinal: usize,
    path: PathBuf,
    size: u64,
}

impl Ord for QuotaEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .modified
            .cmp(&self.modified)
            .then_with(|| other.ordinal.cmp(&self.ordinal))
    }
}

impl PartialOrd for QuotaEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for QuotaEntry {
    fn eq(&self, other: &Self) -> bool {
        self.modified == other.modified && self.ordinal == other.ordinal
    }
}

/// Delete oldest files in `dir` until total size is at or below `max_mb` mebibytes.
pub fn enforce_download_quota(dir: &Path, max_mb: u64) -> anyhow::Result<()> {
    if max_mb == 0 {
        return Ok(());
    }
    enforce_download_quota_bytes(dir, max_mb.saturating_mul(1024 * 1024))
}

/// Delete oldest files until total size is at or below `max_bytes`.
pub fn enforce_download_quota_bytes(dir: &Path, max_bytes: u64) -> anyhow::Result<()> {
    if max_bytes == 0 {
        return Ok(());
    }
    let mut entries = Vec::new();
    let mut total: u64 = 0;

    for (ordinal, dent) in fs::read_dir(dir)?.flatten().enumerate() {
        if !dent.file_type()?.is_file() {
            continue;
        }
        let meta = dent.metadata()?;
        let size = meta.len();
        total = total.saturating_add(size);
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        entries.push(QuotaEntry {
            modified,
            ordinal,
            path: dent.path(),
            size,
        });
    }

    if total <= max_bytes {
        return Ok(());
    }

    let mut oldest_entries = BinaryHeap::from(entries);
    while let Some(entry) = oldest_entries.pop() {
        if total <= max_bytes {
            break;
        }
        if fs::remove_file(&entry.path).is_ok() {
            total = total.saturating_sub(entry.size);
        }
    }
    Ok(())
}
