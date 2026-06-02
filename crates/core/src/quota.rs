use std::fs;
use std::path::Path;
use std::time::SystemTime;

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
    let mut entries: Vec<(SystemTime, std::path::PathBuf, u64)> = Vec::new();
    let mut total: u64 = 0;

    for dent in fs::read_dir(dir)?.flatten() {
        if !dent.file_type()?.is_file() {
            continue;
        }
        let meta = dent.metadata()?;
        let size = meta.len();
        total = total.saturating_add(size);
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        entries.push((modified, dent.path(), size));
    }

    if total <= max_bytes {
        return Ok(());
    }

    entries.sort_by_key(|(mtime, _, _)| *mtime);
    for (_, path, size) in entries {
        if total <= max_bytes {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
    Ok(())
}
