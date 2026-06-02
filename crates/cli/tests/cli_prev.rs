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
}