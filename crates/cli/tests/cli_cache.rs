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
    let walls_state = state_home.join("walls");
    fs::create_dir_all(&walls_config).unwrap();
    fs::create_dir_all(&walls_state).unwrap();

    let cache_dir = tmp.join("cache");
    let download_dir = tmp.join("downloaded");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::create_dir_all(&download_dir).unwrap();

    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": false },
        "paths": {
            "cache_dir": cache_dir.display().to_string(),
            "download_dir": download_dir.display().to_string(),
            "favorites_dir": tmp.join("favorites").display().to_string(),
            "fetched_dir": tmp.join("fetched").display().to_string(),
            "compose_dir": tmp.join("compose").display().to_string(),
        },
        "quota": { "enabled": true, "size_mb": 1 },
        "apply": { "backend": "custom-script", "custom_script": tmp.join("apply.sh").display().to_string() },
        "display": { "mode": "os" },
        "sources": [],
    });
    fs::write(
        walls_config.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(walls_config.join("secrets.json"), "{}").unwrap();
    fs::write(
        walls_state.join("state.json"),
        serde_json::json!({ "cache_queue": ["wallhaven:abc"] }).to_string(),
    )
    .unwrap();
    (config_home, state_home, cache_dir)
}

fn walls_cmd() -> Command {
    Command::new(cargo_bin("walls"))
}

#[test]
fn cli_cache_status_reports_queue_files_and_quota_json() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home, cache_dir) = setup_xdg_home(tmp.path());
    fs::write(cache_dir.join("wallhaven-abc.jpg"), b"abc").unwrap();
    fs::write(cache_dir.join("local-import.jpg"), b"local").unwrap();
    fs::write(
        tmp.path().join("downloaded").join("unsplash-def.jpg"),
        b"data",
    )
    .unwrap();

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["cache", "status", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"command\": \"cache status\""))
        .stdout(predicate::str::contains("\"len\": 1"))
        .stdout(predicate::str::contains("\"provider_files\": 1"))
        .stdout(predicate::str::contains("\"usage_bytes\": 4"));
}

#[test]
fn cli_cache_prune_requires_force_and_dry_run_does_not_mutate() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home, _cache_dir) = setup_xdg_home(tmp.path());
    let state_file = state_home.join("walls").join("state.json");

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["cache", "prune"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "refusing to mutate without --force",
        ));

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["cache", "prune", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("would clear queue: 1 entries"));

    let state = fs::read_to_string(&state_file).unwrap();
    assert!(state.contains("wallhaven:abc"));

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["cache", "prune", "--force", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"cleared_queue\""))
        .stdout(predicate::str::contains("\"queue_cleared\": 1"));

    let state = fs::read_to_string(state_file).unwrap();
    assert!(!state.contains("wallhaven:abc"));
}
