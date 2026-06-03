mod common;

use walls_core::validate::validate_config;
use walls_core::WallsCtx;

#[test]
fn validate_ok_for_minimal_config() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let ctx = WallsCtx::load_from(root.path()).unwrap();
    let errors = validate_config(&ctx.config, &ctx.secrets, &ctx.paths);
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn validate_reports_missing_folder_path() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let config_path = root.path().join("config.json");
    let mut config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    config["sources"] = serde_json::json!([{
        "enabled": true,
        "type": "folder",
        "path": "/nonexistent/walls-test-folder"
    }]);
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    let ctx = WallsCtx::load_from(root.path()).unwrap();
    let errors = validate_config(&ctx.config, &ctx.secrets, &ctx.paths);
    assert!(errors.iter().any(|e| e.contains("does not exist")));
}
