use std::fs;

use walls_core::WallsCtx;

fn write_wallhaven_test_config(root: &std::path::Path, noop_script: &std::path::Path) {
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
        "selection": { "refetch_when_cache_below": 5 },
        "sources": [],
    });
    fs::write(
        root.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(root.join("secrets.json"), "{}").unwrap();
}

#[tokio::test]
async fn advance_next_applies_cached_wallhaven_file() {
    let root = tempfile::tempdir().unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_wallhaven_test_config(root.path(), &noop);

    let cache = root.path().join("cache");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("wallhaven-94x38z.jpg"), b"jpeg").unwrap();

    let state = serde_json::json!({
        "cache_queue": ["94x38z"],
        "history": [],
    });
    fs::write(
        root.path().join("state.json"),
        serde_json::to_string_pretty(&state).unwrap(),
    )
    .unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx.advance_next().await.unwrap().expect("should apply");
    assert!(applied.ends_with("wallhaven-94x38z.jpg"));
    assert!(ctx.state.cache_queue.is_empty());
    assert_eq!(
        ctx.state.current.as_ref().unwrap().wallhaven_id.as_deref(),
        Some("94x38z")
    );
}
