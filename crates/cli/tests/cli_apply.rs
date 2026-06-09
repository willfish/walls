use std::fs;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;

fn setup_xdg_home(
    tmp: &std::path::Path,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let config_home = tmp.join("xdg-config");
    let state_home = tmp.join("xdg-state");
    let walls_config = config_home.join("walls");
    fs::create_dir_all(&walls_config).unwrap();
    fs::create_dir_all(state_home.join("walls")).unwrap();

    let images = tmp.join("images");
    fs::create_dir_all(&images).unwrap();
    let image = images.join("wall.jpg");
    fs::write(&image, b"x").unwrap();

    let marker = tmp.join("backend-ran");
    let backend = tmp.join("backend.sh");
    fs::write(
        &backend,
        format!("#!/bin/sh\necho invoked > '{}'\n", marker.display()),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&backend, fs::Permissions::from_mode(0o755)).unwrap();
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
        "apply": { "backend": "custom-script", "custom_script": backend.display().to_string() },
        "display": { "mode": "os" },
        "sources": [{ "enabled": true, "type": "folder", "path": images.display().to_string() }],
    });
    fs::write(
        walls_config.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(walls_config.join("secrets.json"), "{}").unwrap();
    (config_home, state_home, image, marker)
}

fn walls_cmd() -> Command {
    Command::new(cargo_bin("walls"))
}

#[test]
fn cli_apply_dry_run_reports_plan_without_mutating_state_or_running_backend() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home, image, marker) = setup_xdg_home(tmp.path());
    let state_file = state_home.join("walls/state.json");
    let journal = state_home.join("walls/events.jsonl");

    let assert = walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["apply", "--dry-run", "--json", image.to_str().unwrap()])
        .assert()
        .success();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("apply dry-run json");

    assert_eq!(value["command"], "apply");
    assert_eq!(value["changed"], false);
    assert_eq!(value["status"], "would_apply");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["apply"]["original_exists"], true);
    assert_eq!(value["apply"]["would_run_backend"], true);
    assert_eq!(value["apply"]["would_update_current"], true);
    assert_eq!(value["apply"]["would_update_history"], true);
    assert_eq!(value["apply"]["would_record_event"], true);
    assert_eq!(value["apply"]["resolved_backend"], "custom-script");
    assert!(!marker.exists(), "dry-run must not invoke apply backend");
    assert!(!state_file.exists(), "dry-run must not save state");
    assert!(!journal.exists(), "dry-run must not append events");

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["current", "--json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"status\": \"missing_current\""));
}

#[test]
fn cli_apply_dry_run_reports_missing_original_without_mutating() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home, _image, marker) = setup_xdg_home(tmp.path());
    let missing = tmp.path().join("images/missing.jpg");

    let assert = walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["apply", "--dry-run", "--json", missing.to_str().unwrap()])
        .assert()
        .failure();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("apply dry-run json");

    assert_eq!(value["status"], "missing_original");
    assert_eq!(value["exit_code_reason"], "missing_original");
    assert_eq!(value["apply"]["original_exists"], false);
    assert_eq!(value["apply"]["would_run_backend"], false);
    assert_eq!(value["apply"]["would_update_current"], false);
    assert_eq!(value["apply"]["would_record_event"], false);
    assert!(!marker.exists(), "dry-run must not invoke apply backend");
}

#[test]
fn cli_apply_json_reports_missing_original_without_raw_error() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home, _image, marker) = setup_xdg_home(tmp.path());
    let missing = tmp.path().join("images/missing.jpg");

    let assert = walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["apply", "--json", missing.to_str().unwrap()])
        .assert()
        .failure();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("apply missing json");

    assert_eq!(value["command"], "apply");
    assert_eq!(value["changed"], false);
    assert_eq!(value["status"], "missing_original");
    assert_eq!(value["dry_run"], false);
    assert_eq!(value["exit_code_reason"], "missing_original");
    assert_eq!(value["apply"]["original_exists"], false);
    assert_eq!(value["apply"]["would_run_backend"], false);
    assert!(!marker.exists(), "missing apply must not invoke backend");
}

#[test]
fn cli_apply_human_missing_original_includes_recovery_action() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home, _image, marker) = setup_xdg_home(tmp.path());
    let missing = tmp.path().join("images/missing.jpg");

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["apply", missing.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("wallpaper file does not exist"))
        .stderr(predicate::str::contains("walls next --manual --verbose"));

    assert!(!marker.exists(), "missing apply must not invoke backend");
}

#[test]
fn cli_apply_json_reports_mutating_apply_result() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home, image, marker) = setup_xdg_home(tmp.path());

    let assert = walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["apply", "--json", image.to_str().unwrap()])
        .assert()
        .success();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("apply json");

    assert_eq!(value["command"], "apply");
    assert_eq!(value["changed"], true);
    assert_eq!(value["status"], "applied");
    assert_eq!(value["dry_run"], false);
    assert!(marker.exists(), "real apply should invoke apply backend");
    assert!(state_home.join("walls/state.json").exists());
    assert!(state_home.join("walls/events.jsonl").exists());
}
