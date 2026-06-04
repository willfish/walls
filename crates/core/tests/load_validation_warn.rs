mod common;

use walls_core::validate::{secrets_file_permission_warnings, validate_config};
use walls_core::WallsCtx;

#[test]
fn load_succeeds_when_folder_source_path_missing() {
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

    let ctx = WallsCtx::load_from(root.path()).expect("load should not fail on warnings");
    let errors = validate_config(&ctx.config, &ctx.secrets, &ctx.paths);
    assert!(errors.iter().any(|e| e.contains("does not exist")));
}

#[cfg(unix)]
#[test]
fn warns_when_secrets_file_is_group_or_other_readable() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let secrets = root.path().join("secrets.json");
    std::fs::set_permissions(&secrets, std::fs::Permissions::from_mode(0o644)).unwrap();

    let ctx = WallsCtx::load_from(root.path()).expect("load should not fail on warnings");
    let warnings = secrets_file_permission_warnings(&ctx.paths);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains(&secrets.display().to_string()));
    assert!(warnings[0].contains("chmod 600"));
}

#[cfg(unix)]
#[test]
fn does_not_warn_when_secrets_file_is_private() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    std::fs::set_permissions(
        root.path().join("secrets.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let ctx = WallsCtx::load_from(root.path()).expect("load should not fail");

    assert!(secrets_file_permission_warnings(&ctx.paths).is_empty());
}
