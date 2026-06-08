mod common {
    include!("common/minimal.rs");
}

use walls_core::validate::validate_config;
use walls_core::WallsCtx;

fn load_config_json(root: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root.join("config.json")).unwrap()).unwrap()
}

fn validate_root(root: &std::path::Path) -> Vec<String> {
    let ctx = WallsCtx::load_from(root).unwrap();
    validate_config(&ctx.config, &ctx.secrets, &ctx.paths)
}

#[test]
fn validate_config_ok_for_minimal_config() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let errors = validate_root(root.path());
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn validate_config_reports_missing_folder_path() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["sources"] = serde_json::json!([{
        "enabled": true,
        "type": "folder",
        "path": "/nonexistent/walls-test-folder"
    }]);
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(errors.iter().any(|e| e.contains("does not exist")));
}

#[test]
fn validate_config_reports_missing_unsplash_key() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["change"]["internet_enabled"] = serde_json::json!(true);
    config["sources"] = serde_json::json!([{
        "enabled": true,
        "type": "unsplash",
        "query": "forest"
    }]);
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("secrets.unsplash_access_key is empty")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_missing_custom_script_for_custom_script_backend() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let missing = root.path().join("missing-script.sh");
    let mut config = load_config_json(root.path());
    config["apply"]["custom_script"] = serde_json::json!(missing.display().to_string());
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("apply.custom_script not found or not a file")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_required_custom_script_for_custom_script_backend() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["apply"]["custom_script"] = serde_json::Value::Null;
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors.iter().any(|error| error
            .contains("apply.custom_script is required when apply.backend is custom-script")),
        "{errors:?}"
    );
}

#[cfg(unix)]
#[test]
fn validate_config_reports_non_executable_custom_script_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    std::fs::set_permissions(&noop, std::fs::Permissions::from_mode(0o644)).unwrap();
    common::write_minimal_config(root.path(), &images, &noop);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("apply.custom_script is not executable")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_custom_script_when_backend_does_not_use_it() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["apply"]["backend"] = serde_json::json!("gnome");
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("apply.custom_script is set but apply.backend is gnome")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_zero_quota_size() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["quota"] = serde_json::json!({ "enabled": true, "size_mb": 0 });
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("quota.size_mb must be greater than zero")),
        "{errors:?}"
    );
}
