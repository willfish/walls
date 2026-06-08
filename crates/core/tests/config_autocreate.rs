//! Missing `config.json` is recreated on every load — tray rotation, CLI commands, and TUI reload.

use std::fs;
use std::path::Path;

use walls_core::WallsCtx;

struct HomeGuard {
    previous: Result<String, std::env::VarError>,
}

impl HomeGuard {
    fn set(home: &Path) -> Self {
        let previous = std::env::var("HOME");
        // SAFETY: test-only; config_autocreate tests run serially within this binary.
        unsafe { std::env::set_var("HOME", home) };
        Self { previous }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match &self.previous {
            Ok(value) => unsafe { std::env::set_var("HOME", value) },
            Err(_) => unsafe { std::env::remove_var("HOME") },
        }
    }
}

fn with_isolated_home<F: FnOnce(&Path)>(f: F) {
    let _lock = walls_core::cosmic_theme::lock_env_for_tests();
    let root = tempfile::tempdir().unwrap();
    let _guard = HomeGuard::set(root.path());
    // SAFETY: test-only; guarded by `lock_env_for_tests`.
    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_STATE_HOME");
    }
    f(root.path());
}

#[test]
fn initial_load_creates_default_config() {
    with_isolated_home(|root| {
        let config_path = root.join("config.json");

        assert!(!config_path.exists());
        let ctx = WallsCtx::load_from(root).expect("initial load");

        assert!(config_path.is_file());
        assert!(ctx.config.change.enabled);
    });
}

#[test]
fn reload_recreates_deleted_config() {
    with_isolated_home(|root| {
        let config_path = root.join("config.json");

        let ctx = WallsCtx::load_from(root).expect("seed config");
        fs::remove_file(&config_path).expect("simulate user deleting config.json");
        assert!(!config_path.exists());

        // TUI `reload_ctx` and tray `RotationLoop::poll` both reload via `load_with_paths`.
        let reloaded = WallsCtx::load_with_paths(ctx.paths.clone()).expect("reload after delete");

        assert!(config_path.is_file());
        assert!(reloaded.config.change.enabled);
    });
}
