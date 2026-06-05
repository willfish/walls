mod common;

use std::fs;

use walls_core::apply::ApplyTrigger;
use walls_core::state::{CurrentWall, State};
use walls_core::{RefreshLevel, WallsCtx, WallsError};

#[test]
fn load_from_reports_missing_config_as_typed_error() {
    let root = tempfile::tempdir().unwrap();

    let err = load_error(root.path());

    match err {
        WallsError::ConfigLoad { path, .. } => {
            assert_eq!(path, root.path().join("config.json"));
        }
        other => panic!("expected ConfigLoad, got {other:?}"),
    }
    assert!(err_string_contains(
        &load_error(root.path()),
        "failed to load"
    ));
}

#[test]
fn load_from_reports_invalid_state_as_typed_error() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);
    fs::write(root.path().join("state.json"), "{").unwrap();

    let err = load_error(root.path());

    match err {
        WallsError::StateLoad { path, .. } => {
            assert_eq!(path, root.path().join("state.json"));
        }
        other => panic!("expected StateLoad, got {other:?}"),
    }
}

#[test]
fn apply_file_reports_compose_failures_as_typed_error() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);
    let missing = images.join("missing.jpg");

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let err = ctx
        .apply_file(&missing, ApplyTrigger::Manual)
        .expect_err("missing wallpaper should fail");

    match err {
        WallsError::ApplyFile { original, .. } => {
            assert_eq!(original, missing);
        }
        other => panic!("expected ApplyFile, got {other:?}"),
    }
}

#[test]
fn refresh_current_reports_missing_original_as_typed_error() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);
    let missing = images.join("missing.jpg");

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.state = State {
        current: Some(CurrentWall {
            source_id: "test".to_string(),
            wallhaven_id: None,
            original_path: missing.display().to_string(),
            composed_path: missing.display().to_string(),
            post_filter_path: None,
        }),
        ..State::default()
    };
    ctx.save_state().unwrap();

    let err = ctx
        .refresh_current(RefreshLevel::All)
        .expect_err("missing current original should fail");

    match err {
        WallsError::CurrentOriginalMissing { path } => {
            assert_eq!(path, missing);
        }
        other => panic!("expected CurrentOriginalMissing, got {other:?}"),
    }
}

fn err_string_contains(err: &WallsError, needle: &str) -> bool {
    err.to_string().contains(needle)
}

fn load_error(root: &std::path::Path) -> WallsError {
    match WallsCtx::load_from(root) {
        Ok(_) => panic!("load should fail"),
        Err(err) => err,
    }
}
