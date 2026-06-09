use std::fs;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;

fn setup_xdg_home(tmp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let config_home = tmp.join("xdg-config");
    let state_home = tmp.join("xdg-state");
    let walls_config = config_home.join("walls");
    fs::create_dir_all(&walls_config).unwrap();
    fs::create_dir_all(state_home.join("walls")).unwrap();

    let images = tmp.join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("a.jpg"), b"x").unwrap();

    let noop = tmp.join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": false },
        "paths": {
            "cache_dir": tmp.join("cache").display().to_string(),
            "download_dir": tmp.join("downloaded").display().to_string(),
            "favorites_dir": tmp.join("favorites").display().to_string(),
            "fetched_dir": tmp.join("fetched").display().to_string(),
            "compose_dir": tmp.join("wallpaper").display().to_string(),
        },
        "apply": { "backend": "custom-script", "custom_script": noop.display().to_string() },
        "display": { "mode": "os" },
        "sources": [{ "enabled": true, "type": "folder", "path": images.display().to_string() }],
    });
    fs::write(
        walls_config.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(walls_config.join("secrets.json"), "{}").unwrap();
    (config_home, state_home)
}

fn walls_cmd() -> Command {
    Command::new(cargo_bin("walls"))
}

#[test]
fn cli_current_none_exits_nonzero() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home) = setup_xdg_home(tmp.path());

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .arg("current")
        .assert()
        .failure()
        .stdout(predicate::str::contains("no current wallpaper"))
        .stdout(predicate::str::contains("walls apply <path>"))
        .stdout(predicate::str::contains("walls next --manual"));

    let assert = walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["current", "--json"])
        .assert()
        .failure();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("current missing json");
    assert_eq!(value["command"], "current");
    assert_eq!(value["changed"], false);
    assert_eq!(value["status"], "missing_current");
    assert_eq!(value["current"], serde_json::Value::Null);
    assert_eq!(value["exit_code_reason"], "missing_current");
}

#[test]
fn cli_current_after_apply() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home) = setup_xdg_home(tmp.path());
    let image = tmp.path().join("images/a.jpg");

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["apply", image.to_str().unwrap()])
        .assert()
        .success();

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .arg("current")
        .assert()
        .success()
        .stdout(predicate::str::contains("a.jpg"));

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["current", "--meta"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""original_path""#));

    let assert = walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["current", "--json"])
        .assert()
        .success();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("current json");
    assert_eq!(value["command"], "current");
    assert_eq!(value["changed"], false);
    assert_eq!(value["status"], "current");
    assert!(value["current"]["path"]
        .as_str()
        .unwrap()
        .ends_with("a.jpg"));
    assert!(value["current"]["meta"]["original_path"]
        .as_str()
        .unwrap()
        .ends_with("a.jpg"));
    assert_eq!(value["exit_code_reason"], serde_json::Value::Null);
}
