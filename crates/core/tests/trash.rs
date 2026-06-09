mod common {
    include!("common/minimal.rs");
}

use std::fs;

use walls_core::apply::ApplyTrigger;
use walls_core::WallsCtx;

#[test]
fn trash_current_removes_file_and_clears_state() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let wall = images.join("wall.jpg");
    fs::write(&wall, b"x").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.apply_file(&wall, ApplyTrigger::Manual).unwrap();
    assert!(wall.exists());

    let plan = ctx.plan_trash_current().unwrap();
    assert_eq!(plan.original_path, wall.display().to_string());
    assert!(plan.original_exists);
    assert_eq!(plan.history_entries_removed, 1);

    ctx.trash_current().unwrap();
    assert!(!wall.exists());
    assert!(ctx.state.current.is_none());
    assert!(ctx.state.history.is_empty());
}
