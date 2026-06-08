#[allow(dead_code)]
mod common;

use std::fs;

use walls_core::apply::ApplyTrigger;
use walls_core::WallsCtx;

#[test]
fn favorite_current_copies_to_favorites_dir() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let wall = images.join("wall.jpg");
    fs::write(&wall, b"x").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.apply_file(&wall, ApplyTrigger::Manual).unwrap();

    let dest = ctx.favorite_current().unwrap();
    assert!(dest.starts_with(ctx.paths.favorites_dir));
    assert!(dest.exists());
    assert_ne!(dest, wall);
    let data = fs::read(dest).unwrap();
    assert_eq!(data, b"x");
}

#[test]
fn favorite_picks_unique_name_on_collision() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let wall = images.join("wall.jpg");
    fs::write(&wall, b"x").unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let favorites = root.path().join("favorites");
    fs::create_dir_all(&favorites).unwrap();
    fs::write(favorites.join("wall.jpg"), b"old").unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.apply_file(&wall, ApplyTrigger::Manual).unwrap();
    let dest = ctx.favorite_current().unwrap();
    assert_ne!(dest, favorites.join("wall.jpg"));
    assert!(dest.file_name().unwrap().to_str().unwrap().contains("wall"));
}
