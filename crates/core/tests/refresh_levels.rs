mod common;

use std::fs;

use walls_core::apply::ApplyTrigger;
use walls_core::{RefreshLevel, WallsCtx};

#[test]
fn refresh_all_reapplies_current_without_advancing_history() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let first = images.join("first.jpg");
    let second = images.join("second.jpg");
    fs::write(&first, b"a").unwrap();
    fs::write(&second, b"b").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.apply_file(&first, ApplyTrigger::Manual).unwrap();
    ctx.apply_file(&second, ApplyTrigger::Manual).unwrap();
    let history_before = ctx.state.history.clone();

    let refreshed = ctx
        .refresh_current(RefreshLevel::All)
        .unwrap()
        .expect("current wallpaper refreshed");

    assert_eq!(refreshed, second);
    assert_eq!(ctx.state.history, history_before);
    assert_eq!(ctx.state.history_index, 0);
    assert_eq!(
        ctx.state
            .current
            .as_ref()
            .unwrap()
            .post_filter_path
            .as_deref(),
        Some(second.display().to_string().as_str())
    );
}

#[test]
fn refresh_clock_only_reuses_current_composed_file() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let image = images.join("wall.jpg");
    fs::write(&image, b"a").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.apply_file(&image, ApplyTrigger::Manual).unwrap();
    let current_before = ctx.state.current.clone().unwrap();

    let refreshed = ctx
        .refresh_current(RefreshLevel::ClockOnly)
        .unwrap()
        .expect("current wallpaper refreshed");

    assert_eq!(refreshed, image);
    assert_eq!(ctx.state.history, vec![image.display().to_string()]);
    assert_eq!(
        ctx.state.current.as_ref().unwrap().composed_path,
        current_before.composed_path
    );
}

#[test]
fn refresh_without_current_wallpaper_is_noop() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();

    assert!(ctx.refresh_current(RefreshLevel::Texts).unwrap().is_none());
}
