use std::fs::{File, OpenOptions};
use std::path::Path;

use fs2::FileExt;

/// Advisory exclusive lock on `state.json` for the lifetime of this guard.
pub struct StateLock {
    _file: File,
}

impl StateLock {
    pub fn acquire(state_file: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(state_file)?;
        file.lock_exclusive()?;
        Ok(Self { _file: file })
    }
}
