mod common {
    include!("common/minimal.rs");
}

use std::fs;
use std::path::{Path, PathBuf};

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
fn text_only_refresh_skips_compose_but_all_refresh_recomposes() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let image = images.join("wall.jpg");
    fs::write(&image, b"a").unwrap();
    let noop = common::write_noop_script(root.path());
    let compose_log = root.path().join("compose.log");
    let compose_script = write_compose_script(root.path(), &compose_log);
    write_composing_config(root.path(), &images, &noop, &compose_script);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.apply_file(&image, ApplyTrigger::Manual).unwrap();
    let current_before = ctx.state.current.clone().unwrap();
    let composed_before = PathBuf::from(&current_before.composed_path);
    let modified_before = fs::metadata(&composed_before).unwrap().modified().unwrap();
    assert_eq!(compose_count(&compose_log), 1);

    let text_refreshed = ctx
        .refresh_current(RefreshLevel::Texts)
        .unwrap()
        .expect("current wallpaper refreshed");

    assert_eq!(text_refreshed, composed_before);
    assert_eq!(compose_count(&compose_log), 1);
    assert_eq!(
        fs::metadata(&composed_before).unwrap().modified().unwrap(),
        modified_before
    );
    assert_eq!(
        ctx.state.current.as_ref().unwrap().composed_path,
        current_before.composed_path
    );

    let all_refreshed = ctx
        .refresh_current(RefreshLevel::All)
        .unwrap()
        .expect("current wallpaper refreshed");

    assert_eq!(all_refreshed, composed_before);
    assert_eq!(compose_count(&compose_log), 2);
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

fn write_composing_config(
    root: &Path,
    image_dir: &Path,
    apply_script: &Path,
    compose_script: &Path,
) {
    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": false },
        "paths": {
            "cache_dir": root.join("cache").display().to_string(),
            "download_dir": root.join("downloaded").display().to_string(),
            "favorites_dir": root.join("favorites").display().to_string(),
            "fetched_dir": root.join("fetched").display().to_string(),
            "compose_dir": root.join("wallpaper").display().to_string(),
        },
        "apply": {
            "backend": "custom-script",
            "custom_script": apply_script.display().to_string(),
        },
        "display": {
            "mode": "zoom",
            "imagemagick_command": compose_script.display().to_string(),
            "target_width": 1920,
            "target_height": 1080,
        },
        "selection": { "avoid_recent": 50 },
        "sources": [
            { "enabled": true, "type": "folder", "path": image_dir.display().to_string() }
        ],
    });
    fs::write(
        root.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(root.join("secrets.json"), "{}").unwrap();
}

fn write_compose_script(root: &Path, log: &Path) -> PathBuf {
    let script = root.join("compose.sh");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\ninput=$1\nlast=\nfor arg in \"$@\"; do\n  last=$arg\ndone\nprintf 'compose\\n' >> '{}'\ncp \"$input\" \"$last\"\n",
            log.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    script
}

fn compose_count(log: &Path) -> usize {
    fs::read_to_string(log).unwrap().lines().count()
}
