use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use walls_core::WallsCtx;

pub fn write_noop_script(root: &Path) -> PathBuf {
    let noop = root.join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    noop
}

pub fn write_minimal_config(root: &Path, image_dir: &Path, noop: &Path) {
    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": false },
        "paths": {
            "cache_dir": root.join("cache").display().to_string(),
            "download_dir": root.join("downloaded").display().to_string(),
            "favorites_dir": root.join("favorites").display().to_string(),
            "fetched_dir": root.join("fetched").display().to_string(),
            "compose_dir": root.join("wallpaper").display().to_string(),
        },
        "apply": {
            "backend": "custom-script",
            "custom_script": noop.display().to_string(),
        },
        "display": { "mode": "os" },
        "selection": { "avoid_recent": 50 },
        "sources": [
            { "enabled": true, "type": "folder", "path": image_dir.display().to_string() }
        ],
    });
    write_config(root, config);
    write_secrets(root, json!({}));
}

pub fn write_config(root: &Path, config: Value) {
    fs::write(
        root.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
}

pub fn write_secrets(root: &Path, secrets: Value) {
    fs::write(
        root.join("secrets.json"),
        serde_json::to_string_pretty(&secrets).unwrap(),
    )
    .unwrap();
}

pub fn write_state(root: &Path, state: Value) {
    fs::write(
        root.join("state.json"),
        serde_json::to_string_pretty(&state).unwrap(),
    )
    .unwrap();
}

pub fn ensure_local_dirs(root: &Path) {
    for dir in ["cache", "downloaded", "favorites", "fetched", "wallpaper"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
}

pub fn write_image(root: &Path, relative: &str, bytes: &[u8]) -> PathBuf {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, bytes).unwrap();
    path
}

pub fn paths_block(root: &Path) -> Value {
    json!({
        "cache_dir": root.join("cache").display().to_string(),
        "download_dir": root.join("downloaded").display().to_string(),
        "favorites_dir": root.join("favorites").display().to_string(),
        "fetched_dir": root.join("fetched").display().to_string(),
        "compose_dir": root.join("wallpaper").display().to_string(),
    })
}

pub fn apply_block(noop: &Path) -> Value {
    json!({
        "backend": "custom-script",
        "custom_script": noop.display().to_string(),
    })
}

/// Minimal hermetic harness: temp root, noop apply script, standard dirs.
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
            "sources": sources,
        })
    }
}
