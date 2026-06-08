use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use walls_core::WallsCtx;

include!("files.rs");

fn ensure_local_dirs(root: &Path) {
    for dir in ["cache", "downloaded", "favorites", "fetched", "wallpaper"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
}

fn write_state(root: &Path, state: Value) {
    fs::write(
        root.join("state.json"),
        serde_json::to_string_pretty(&state).unwrap(),
    )
    .unwrap();
}

pub fn write_image(root: &Path, relative: &str, bytes: &[u8]) -> PathBuf {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, bytes).unwrap();
    path
}

/// Hermetic harness: temp root, noop apply script, standard dirs.
pub struct FetchHarness {
    pub root: tempfile::TempDir,
    pub noop: PathBuf,
}

impl FetchHarness {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        ensure_local_dirs(root.path());
        let noop = write_noop_script(root.path());
        Self { root, noop }
    }

    pub fn path(&self) -> &Path {
        self.root.path()
    }

    pub fn write_config(&self, config: Value) {
        write_config(self.path(), config);
    }

    pub fn write_secrets(&self, secrets: Value) {
        write_secrets(self.path(), secrets);
    }

    pub fn write_state(&self, state: Value) {
        write_state(self.path(), state);
    }

    pub fn write_cache_file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        write_image(self.path(), &format!("cache/{name}"), bytes)
    }

    pub fn load_ctx(&self) -> WallsCtx {
        WallsCtx::load_from(self.path()).unwrap()
    }

    pub fn base_config(&self, internet_enabled: bool, sources: Value) -> Value {
        json!({
            "change": { "enabled": true, "internet_enabled": internet_enabled },
            "paths": paths_block(self.path()),
            "apply": apply_block(&self.noop),
            "display": { "mode": "os" },
            "selection": { "avoid_recent": 50, "refetch_when_cache_below": 5 },
            "wallhaven": { "enabled": false },
            "sources": sources,
        })
    }

    pub fn write_offline_empty_sources_config(&self) {
        self.write_config(json!({
            "change": { "enabled": true, "internet_enabled": false },
            "paths": paths_block(self.path()),
            "apply": apply_block(&self.noop),
            "display": { "mode": "os" },
            "selection": { "refetch_when_cache_below": 5 },
            "sources": [],
        }));
        self.write_secrets(json!({}));
    }
}
