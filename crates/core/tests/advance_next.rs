use std::fs;

use walls_core::WallsCtx;

fn write_test_config(
    root: &std::path::Path,
    image_dir: &std::path::Path,
    noop_script: &std::path::Path,
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
            "custom_script": noop_script.display().to_string(),
        },
        "display": { "mode": "os" },
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

#[tokio::test]
async fn advance_next_writes_state() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("wall.jpg"), b"fake jpeg").unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_test_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx
        .advance_next()
        .await
        .unwrap()
        .expect("should apply one image");
    assert!(applied.ends_with("wall.jpg"));
    assert!(ctx.state.current.is_some());
    assert_eq!(ctx.state.history.len(), 1);
    assert_eq!(ctx.state.history[0], applied.display().to_string());
}

#[tokio::test]
async fn advance_next_returns_none_when_local_sources_are_empty() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_test_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx.advance_next().await.unwrap();

    assert!(applied.is_none());
    assert!(ctx.state.current.is_none());
    assert!(ctx.state.history.is_empty());
}

#[tokio::test]
async fn advance_next_avoids_recent_candidates_across_many_local_files() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("recent-a.jpg"), b"fake jpeg").unwrap();
    fs::write(images.join("recent-b.jpg"), b"fake jpeg").unwrap();
    fs::write(images.join("available.jpg"), b"fake jpeg").unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_test_config(root.path(), &images, &noop);
    fs::write(
        root.path().join("state.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "history": [
                images.join("recent-a.jpg").display().to_string(),
                images.join("recent-b.jpg").display().to_string()
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx
        .advance_next()
        .await
        .unwrap()
        .expect("should apply the only non-recent image");

    assert!(applied.ends_with("available.jpg"));
    assert_eq!(ctx.state.history[0], applied.display().to_string());
}
