use std::fs;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;

fn setup_xdg_home(
    tmp: &std::path::Path,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let config_home = tmp.join("xdg-config");
    let state_home = tmp.join("xdg-state");
    let walls_config = config_home.join("walls");
    fs::create_dir_all(&walls_config).unwrap();
    fs::create_dir_all(state_home.join("walls")).unwrap();

    let images = tmp.join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("wall.jpg"), b"x").unwrap();

    let log = tmp.join("apply.log");
    let apply_script = tmp.join("apply.sh");
    fs::write(
        &apply_script,
        format!(
            "#!/bin/sh\nprintf '%s|%s|%s|%s\\n' \"$1\" \"$2\" \"$3\" \"$4\" >> {}\n",
            log.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&apply_script, fs::Permissions::from_mode(0o755)).unwrap();
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
        "apply": { "backend": "custom-script", "custom_script": apply_script.display().to_string() },
        "display": { "mode": "os" },
        "sources": [{ "enabled": true, "type": "folder", "path": images.display().to_string() }],
    });
    fs::write(
        walls_config.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(walls_config.join("secrets.json"), "{}").unwrap();
    (config_home, state_home, log)
}

fn walls_cmd() -> Command {
    Command::new(cargo_bin("walls"))
}

#[test]
fn cli_next_refresh_reapplies_current_wallpaper() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home, log) = setup_xdg_home(tmp.path());
    let image = tmp.path().join("images/wall.jpg");

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["apply", image.to_str().unwrap()])
        .assert()
        .success();

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["next", "--refresh", "clock-only"])
        .assert()
        .success()
        .stdout(predicate::str::contains("wall.jpg"));

    let log = fs::read_to_string(log).unwrap();
    assert!(log.lines().any(|line| line.contains("|manual|")));
    assert!(log.lines().any(|line| line.contains("|refresh|")));
}

#[test]
fn cli_next_refresh_without_current_wallpaper_reports_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home, _log) = setup_xdg_home(tmp.path());

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["next", "--refresh", "texts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no current wallpaper"));
}
