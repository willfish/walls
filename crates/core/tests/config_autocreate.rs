//! Missing `config.json` is recreated on every load — tray rotation, CLI commands, and TUI reload.

use std::fs;

use walls_core::WallsCtx;

#[test]
fn initial_load_creates_default_config() {
    let root = tempfile::tempdir().unwrap();
    let config_path = root.path().join("config.json");

    assert!(!config_path.exists());
    let ctx = WallsCtx::load_from(root.path()).expect("initial load");

    assert!(config_path.is_file());
    assert!(ctx.config.change.enabled);
}

#[test]
fn reload_recreates_deleted_config() {
    let root = tempfile::tempdir().unwrap();
    let config_path = root.path().join("config.json");

    let ctx = WallsCtx::load_from(root.path()).expect("seed config");
    fs::remove_file(&config_path).expect("simulate user deleting config.json");
    assert!(!config_path.exists());

    // TUI `reload_ctx` and tray `RotationLoop::poll` both reload via `load_with_paths`.
    let reloaded = WallsCtx::load_with_paths(ctx.paths.clone()).expect("reload after delete");

    assert!(config_path.is_file());
    assert!(reloaded.config.change.enabled);
}
