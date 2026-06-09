use std::fs;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;

fn setup_xdg_home(tmp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let config_home = tmp.join("xdg-config");
    let state_home = tmp.join("xdg-state");
    let walls_config = config_home.join("walls");
    let walls_state = state_home.join("walls");
    fs::create_dir_all(&walls_config).unwrap();
    fs::create_dir_all(&walls_state).unwrap();

    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": false },
        "paths": {
            "cache_dir": tmp.join("cache").display().to_string(),
            "download_dir": tmp.join("downloaded").display().to_string(),
            "favorites_dir": tmp.join("favorites").display().to_string(),
            "fetched_dir": tmp.join("fetched").display().to_string(),
            "compose_dir": tmp.join("compose").display().to_string(),
        },
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
    (config_home, state_home)
}

fn write_journal(state_home: &std::path::Path) {
    let journal = state_home.join("walls").join("events.jsonl");
    let events = [
        serde_json::json!({
            "timestamp_unix": 100,
            "kind": "apply",
            "trigger": "manual",
            "original_path": "/walls/one.jpg",
            "composed_path": "/walls/composed-one.jpg",
            "provider": "local",
        }),
        serde_json::json!({
            "timestamp_unix": 200,
            "kind": "provider_attempt",
            "attempt": {
                "provider_id": "wallhaven",
                "provider_kind": "wallhaven",
                "operation": "advance_next",
                "status": "enabled",
                "retries": [],
                "outcome": {
                    "result": "failed",
                    "kind": "request",
                    "status_code": 401,
                    "message": "[redacted]"
                },
                "fallback_provider_id": null
            }
        }),
        serde_json::json!({
            "timestamp_unix": 300,
            "kind": "provider_attempt",
            "attempt": {
                "provider_id": "unsplash",
                "provider_kind": "unsplash",
                "operation": "advance_next",
                "status": "credential_missing",
                "retries": [],
                "outcome": {
                    "result": "skipped",
                    "reason": "credential_missing"
                },
                "fallback_provider_id": null
            }
        }),
    ];
    let lines = events
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");
    fs::write(journal, format!("{lines}\n")).unwrap();
}

fn walls_cmd() -> Command {
    Command::new(cargo_bin("walls"))
}

#[test]
fn cli_logs_reports_recent_events_newest_first() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home) = setup_xdg_home(tmp.path());
    write_journal(&state_home);

    let assert = walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["logs", "--tail", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("300\twarn\tprovider\tunsplash"))
        .stdout(predicate::str::contains("200\terror\tprovider\twallhaven"));

    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(
        output.find("300\twarn").unwrap() < output.find("200\terror").unwrap(),
        "{output}"
    );
    assert!(!output.contains("100\tinfo\tapply"), "{output}");
}

#[test]
fn cli_logs_json_filters_provider_level_and_since_without_leaking_secrets() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home) = setup_xdg_home(tmp.path());
    write_journal(&state_home);

    let assert = walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args([
            "logs",
            "--json",
            "--provider",
            "wallhaven",
            "--level",
            "error",
            "--since",
            "150",
        ])
        .assert()
        .success();
    let value: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("logs json");

    assert_eq!(value["command"], "logs");
    assert_eq!(value["changed"], false);
    assert_eq!(value["status"], "ok");
    assert_eq!(value["filters"]["provider"], "wallhaven");
    assert_eq!(value["filters"]["level"], "error");
    assert_eq!(value["filters"]["since"], 150);
    assert_eq!(value["events"].as_array().unwrap().len(), 1);
    assert_eq!(value["events"][0]["level"], "error");
    assert_eq!(value["events"][0]["attempt"]["provider_id"], "wallhaven");
    let raw = serde_json::to_string(&value).unwrap();
    assert!(!raw.contains("super-secret-token"), "{raw}");
}

#[test]
fn cli_logs_empty_journal_is_actionable() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home) = setup_xdg_home(tmp.path());

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .arg("logs")
        .assert()
        .success()
        .stdout(predicate::str::contains("no log events"))
        .stdout(predicate::str::contains("walls next --manual"));
}

#[test]
fn cli_logs_rejects_invalid_level() {
    let tmp = tempfile::tempdir().unwrap();
    let (config_home, state_home) = setup_xdg_home(tmp.path());

    walls_cmd()
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_STATE_HOME", &state_home)
        .args(["logs", "--level", "loud"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}
