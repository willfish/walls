use std::fs::{File, OpenOptions};
use std::path::Path;

use fs2::FileExt;

/// Guard for the tray singleton lock. Held for the lifetime of the tray process.
pub struct TrayLock {
    _file: File,
}

/// Try to acquire an exclusive lock for the tray process.
/// Returns Err if another instance holds the lock (for early exit / singleton).
pub fn try_acquire_tray_lock(path: &Path) -> anyhow::Result<TrayLock> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock_exclusive()?;
    Ok(TrayLock { _file: file })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use std::fs;

    #[test]
    fn second_tray_instance_detects_lock_and_exits_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("tray.lock");

        // Simulate another instance holding the lock.
        let _held = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        _held.try_lock_exclusive().unwrap();

        // Now try to acquire - should fail (for singleton early exit).
        let result = try_acquire_tray_lock(&lock_path);
        assert!(
            result.is_err(),
            "expected lock acquisition to fail when held by another instance"
        );
    }
}
