use std::fs;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cli_prev_walks_history() {
    let tmp = tempfile::tempdir().unwrap();
    let config_home = tmp.path().join("xdg-config");
    let state_home = tmp.path().join("xdg-state");
    let walls_config = config_home.join("walls");
    fs::create_dir_all(&walls_config).unwrap();
    fs::create_dir_all(state_home.join("walls")).unwrap();

    let images = tmp.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let first = images.join("first.jpg");
    let second = images.join("second.jpg");
    fs::write(&first, b"a").unwrap();
    fs::write(&second, b"b").unwrap();

    let noop = tmp.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true },
        "paths": {
            "cache_dir": tmp.path().join("cache").display().to_string(),
            "download_dir": tmp.path().join("downloaded").display().to_string(),
            "favorites_dir": tmp.path().join("favorites").display().to_string(),
            "fetched_dir": tmp.path().join("fetched").display().to_string(),
            "compose_dir": tmp.path().join("wallpaper").display().to_string(),
        },
        "apply": { "backend": "custom-script", "custom_script": noop.display().to_string() },
        "display": { "mode": "os" },
        "sources": [],
    });
    fs::write(
        walls_config.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(walls_config.join("secrets.json"), "{}").unwrap();

    let env = |cmd: &mut Command| {
        cmd.env("XDG_CONFIG_HOME", &config_home)
            .env("XDG_STATE_HOME", &state_home);
    };

    let mut c = Command::new(cargo_bin("walls"));
    env(&mut c);
    c.args(["apply", &first.display().to_string()])
        .assert()
        .success();

    let mut c = Command::new(cargo_bin("walls"));
    env(&mut c);
    c.args(["apply", &second.display().to_string()])
        .assert()
        .success();

    let mut c = Command::new(cargo_bin("walls"));
    env(&mut c);
    c.arg("prev")
        .assert()
        .success()
        .stdout(predicate::str::contains("first.jpg"));

    let mut c = Command::new(cargo_bin("walls"));
    env(&mut c);
    let assert = c.args(["prev", "--json"]).assert().success();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("prev json");
    assert_eq!(value["command"], "prev");
    assert_eq!(value["changed"], true);
    assert_eq!(value["status"], "applied_previous");
    assert!(value["path"].as_str().unwrap().ends_with(".jpg"));
    assert_eq!(value["exit_code_reason"], serde_json::Value::Null);
}

#[test]
fn cli_undo_restores_previous_history_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let config_home = tmp.path().join("xdg-config");
    let state_home = tmp.path().join("xdg-state");
    let walls_config = config_home.join("walls");
    fs::create_dir_all(&walls_config).unwrap();
    fs::create_dir_all(state_home.join("walls")).unwrap();

    let images = tmp.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let first = images.join("first.jpg");
    let second = images.join("second.jpg");
    fs::write(&first, b"a").unwrap();
    fs::write(&second, b"b").unwrap();

    let noop = tmp.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true },
        "paths": {
            "cache_dir": tmp.path().join("cache").display().to_string(),
            "download_dir": tmp.path().join("downloaded").display().to_string(),
            "favorites_dir": tmp.path().join("favorites").display().to_string(),
            "fetched_dir": tmp.path().join("fetched").display().to_string(),
            "compose_dir": tmp.path().join("wallpaper").display().to_string(),
        },
        "apply": { "backend": "custom-script", "custom_script": noop.display().to_string() },
        "display": { "mode": "os" },
        "sources": [],
    });
    fs::write(
        walls_config.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(walls_config.join("secrets.json"), "{}").unwrap();

    for image in [&first, &second] {
        Command::new(cargo_bin("walls"))
            .env("XDG_CONFIG_HOME", &config_home)
            .env("XDG_STATE_HOME", &state_home)
            .args(["apply", &image.display().to_string()])
            .assert()
            .success();
    }

    let assert = Command::new(cargo_bin("walls"))
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["undo", "--json"])
        .assert()
        .success();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("undo json");
    assert_eq!(value["command"], "undo");
    assert_eq!(value["changed"], true);
    assert_eq!(value["status"], "restored_previous");
    assert!(value["path"].as_str().unwrap().ends_with("first.jpg"));
}

#[test]
fn cli_prev_json_reports_missing_history_file() {
    let tmp = tempfile::tempdir().unwrap();
    let config_home = tmp.path().join("xdg-config");
    let state_home = tmp.path().join("xdg-state");
    let walls_config = config_home.join("walls");
    fs::create_dir_all(&walls_config).unwrap();
    fs::create_dir_all(state_home.join("walls")).unwrap();

    let images = tmp.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let first = images.join("first.jpg");
    let second = images.join("second.jpg");
    fs::write(&first, b"a").unwrap();
    fs::write(&second, b"b").unwrap();

    let noop = tmp.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true },
        "paths": {
            "cache_dir": tmp.path().join("cache").display().to_string(),
            "download_dir": tmp.path().join("downloaded").display().to_string(),
            "favorites_dir": tmp.path().join("favorites").display().to_string(),
            "fetched_dir": tmp.path().join("fetched").display().to_string(),
            "compose_dir": tmp.path().join("wallpaper").display().to_string(),
        },
        "apply": { "backend": "custom-script", "custom_script": noop.display().to_string() },
        "display": { "mode": "os" },
        "sources": [],
    });
    fs::write(
        walls_config.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(walls_config.join("secrets.json"), "{}").unwrap();

    for image in [&first, &second] {
        Command::new(cargo_bin("walls"))
            .env("XDG_CONFIG_HOME", &config_home)
            .env("XDG_STATE_HOME", &state_home)
            .args(["apply", &image.display().to_string()])
            .assert()
            .success();
    }
    fs::remove_file(&first).unwrap();

    let assert = Command::new(cargo_bin("walls"))
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["prev", "--json"])
        .assert()
        .failure();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("prev json");
    assert_eq!(value["command"], "prev");
    assert_eq!(value["changed"], false);
    assert_eq!(value["status"], "missing_previous");
    assert_eq!(value["exit_code_reason"], "missing_previous");
    assert!(value["path"].as_str().unwrap().ends_with("first.jpg"));
}
