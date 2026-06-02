use std::fs;

use walls_core::WallsCtx;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn advance_next_downloads_when_queued_but_not_cached() {
    let server = MockServer::start().await;
    let image_url = format!("{}/wallhaven-94x38z.jpg", server.uri());

    Mock::given(method("GET"))
        .and(path("/api/v1/w/94x38z"))
        .and(header("X-API-Key", "key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "id": "94x38z", "path": image_url }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/wallhaven-94x38z.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"jpeg"))
        .mount(&server)
        .await;

    std::env::set_var("WALLHAVEN_API_BASE", server.uri());

    let root = tempfile::tempdir().unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": true },
        "paths": {
            "cache_dir": root.path().join("cache").display().to_string(),
            "download_dir": root.path().join("downloaded").display().to_string(),
            "favorites_dir": root.path().join("favorites").display().to_string(),
            "fetched_dir": root.path().join("fetched").display().to_string(),
            "compose_dir": root.path().join("wallpaper").display().to_string(),
        },
        "apply": { "backend": "custom-script", "custom_script": noop.display().to_string() },
        "display": { "mode": "os" },
        "selection": { "refetch_when_cache_below": 5 },
        "sources": [],
    });
    fs::write(
        root.path().join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(
        root.path().join("secrets.json"),
        r#"{"wallhaven_api_key":"key"}"#,
    )
    .unwrap();
    fs::write(
        root.path().join("state.json"),
        r#"{"cache_queue":["94x38z"]}"#,
    )
    .unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx.advance_next().await.unwrap().expect("applied");
    assert!(applied.exists());
    assert!(ctx.state.cache_queue.is_empty());
}