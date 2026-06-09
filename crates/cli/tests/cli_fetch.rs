use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;

fn walls_cmd() -> Command {
    Command::new(cargo_bin("walls"))
}

#[test]
fn cli_fetch_without_paths_reports_recovery_without_loading_config() {
    let tmp = tempfile::tempdir().unwrap();

    walls_cmd()
        .env("XDG_CONFIG_HOME", tmp.path().join("missing-config-home"))
        .env("XDG_STATE_HOME", tmp.path().join("missing-state-home"))
        .arg("fetch")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "fetch requires at least one image path",
        ))
        .stderr(predicate::str::contains("walls fetch <path>"));
}
