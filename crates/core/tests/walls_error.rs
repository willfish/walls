mod common {
    include!("common/minimal.rs");
}

use std::fs;

use walls_core::apply::ApplyTrigger;
use walls_core::events::{read_events, EventKind};
use walls_core::state::{CurrentWall, State};
use walls_core::{RefreshLevel, WallsCtx, WallsError};

#[test]
fn load_from_creates_default_config_when_missing() {
    let root = tempfile::tempdir().unwrap();
    let config_path = root.path().join("config.json");

    let ctx = WallsCtx::load_from(root.path()).expect("missing config should be created");

    assert!(config_path.is_file(), "config.json should be written");
    assert!(ctx.config.change.enabled);
    assert_eq!(ctx.config.paths.cache_dir, "~/.local/share/walls/cache");
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
fn apply_file_records_backend_failures_in_event_journal() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let wall = images.join("wall.jpg");
    fs::write(&wall, b"x").unwrap();
    let failing = root.path().join("failing.sh");
    fs::write(
        &failing,
        "#!/bin/sh\nSECRET_MARKER=backend-token-secret\nexit 42\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&failing, fs::Permissions::from_mode(0o755)).unwrap();
    }
    common::write_minimal_config(root.path(), &images, &failing);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let err = ctx
        .apply_file(&wall, ApplyTrigger::Manual)
        .expect_err("backend failure should fail apply");
    assert!(matches!(err, WallsError::ApplyFile { .. }));
    let message = err.to_string();
    assert!(
        message.contains("custom-script apply backend failed"),
        "{message}"
    );
    assert!(message.contains("walls config validate"), "{message}");

    let events = read_events(&ctx.paths.event_journal_file).unwrap();
    assert!(matches!(
        events.as_slice(),
        [walls_core::events::EventRecord {
            kind: EventKind::ApplyFailed {
                trigger: ApplyTrigger::Manual,
                ..
            },
            ..
        }]
    ));
    let raw = fs::read_to_string(&ctx.paths.event_journal_file).unwrap();
    assert!(!raw.contains("backend-token-secret"), "{raw}");
}

#[test]
fn apply_file_reports_missing_custom_script_with_recovery_hint() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let wall = images.join("wall.jpg");
    fs::write(&wall, b"x").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.config.apply.custom_script = None;
    let err = ctx
        .apply_file(&wall, ApplyTrigger::Manual)
        .expect_err("missing custom script should fail apply");

    let message = err.to_string();
    assert!(
        message.contains("apply.custom_script is not set"),
        "{message}"
    );
    assert!(message.contains("existing executable script"), "{message}");
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
            provider: None,
            source_url: None,
            author: None,
            description: None,
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

fn load_error(root: &std::path::Path) -> WallsError {
    match WallsCtx::load_from(root) {
        Ok(_) => panic!("load should fail"),
        Err(err) => err,
    }
}
