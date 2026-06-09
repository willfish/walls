mod common {
    include!("common/minimal.rs");
}

use std::fs;

use walls_core::apply::ApplyTrigger;
use walls_core::{WallsCtx, WallsError};

#[tokio::test]
async fn advance_prev_returns_older_history_entry() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("first.jpg"), b"a").unwrap();
    fs::write(images.join("second.jpg"), b"b").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let first = images.join("first.jpg");
    let second = images.join("second.jpg");
    ctx.apply_file(&first, ApplyTrigger::Manual).unwrap();
    ctx.apply_file(&second, ApplyTrigger::Manual).unwrap();
    assert_eq!(ctx.state.history_index, 0);
    assert_eq!(ctx.state.history[0], second.display().to_string());

    let prev = ctx.advance_prev().unwrap().expect("previous wallpaper");
    assert_eq!(prev, first);
    assert_eq!(ctx.state.history_index, 1);
}

#[tokio::test]
async fn advance_prev_reports_missing_history_file_without_advancing_index() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("first.jpg"), b"a").unwrap();
    fs::write(images.join("second.jpg"), b"b").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let first = images.join("first.jpg");
    let second = images.join("second.jpg");
    ctx.apply_file(&first, ApplyTrigger::Manual).unwrap();
    ctx.apply_file(&second, ApplyTrigger::Manual).unwrap();
    fs::remove_file(&first).unwrap();

    let error = ctx
        .advance_prev()
        .expect_err("missing previous should fail");
    assert!(matches!(
        error,
        WallsError::PreviousOriginalMissing { path } if path == first
    ));
    assert_eq!(ctx.state.history_index, 0);
}
