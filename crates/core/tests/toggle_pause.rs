mod common;

use walls_core::WallsCtx;

#[test]
fn toggle_pause_flips_state() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    assert!(!ctx.state.paused);
    ctx.toggle_pause().unwrap();
    assert!(ctx.state.paused);
    ctx.toggle_pause().unwrap();
    assert!(!ctx.state.paused);
}