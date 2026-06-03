mod common;

use std::fs;
use std::path::PathBuf;

use walls_core::WallsCtx;

#[test]
fn fetch_copies_into_fetched_dir() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let src = images.join("inbox.jpg");
    fs::write(&src, b"x").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let ctx = WallsCtx::load_from(root.path()).unwrap();
    let imported = ctx.fetch_files(&[PathBuf::from(&src)], false).unwrap();
    assert_eq!(imported.len(), 1);
    assert!(imported[0].starts_with(ctx.paths.fetched_dir));
    assert!(imported[0].exists());
    assert!(src.exists());
}

#[test]
fn fetch_move_removes_source() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let src = images.join("inbox.jpg");
    fs::write(&src, b"x").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let ctx = WallsCtx::load_from(root.path()).unwrap();
    let imported = ctx.fetch_files(&[PathBuf::from(&src)], true).unwrap();
    assert!(imported[0].exists());
    assert!(!src.exists());
}
