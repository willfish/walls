use std::fs::{File, OpenOptions};
use std::path::Path;

use fs2::FileExt;

/// Reusable process singleton lock (advisory exclusive on a file for lifetime of guard).
/// Used by StateLock and tray singleton.
pub struct ProcessLock {
    _file: File,
}

impl ProcessLock {
    pub fn acquire(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.lock_exclusive()?;
        Ok(Self { _file: file })
    }

    pub fn try_acquire(path: &Path) -> anyhow::Result<Self> {
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
        Ok(Self { _file: file })
    }
}

/// Advisory exclusive lock on `state.json` for the lifetime of this guard.
pub struct StateLock {
    _inner: ProcessLock,
}

impl StateLock {
    pub fn acquire(state_file: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            _inner: ProcessLock::acquire(state_file)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn process_lock_acquire_and_try_acquire_work() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.lock");

        let _lock = ProcessLock::acquire(&path).unwrap();

        // try when held should fail (for singleton use like tray)
        let result = ProcessLock::try_acquire(&path);
        assert!(result.is_err());
    }
}
