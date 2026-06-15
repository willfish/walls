use std::fs;

use walls_core::providers::ProviderOperation;
use walls_core::WallsCtx;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn advance_next_downloads_queued_unsplash_photo_and_records_metadata() {
    let server = MockServer::start().await;
    let image_url = format!("{}/images/abc123.jpg", server.uri());
    let download_location = format!("{}/photos/abc123/download", server.uri());

    Mock::given(method("GET"))
        .and(path("/photos/abc123"))
        .and(header("Authorization", "Client-ID key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "abc123",
            "urls": {
                "raw": image_url,
                "full": image_url,
                "regular": image_url
            },
            "links": {
                "html": "https://unsplash.com/photos/abc123",
                "download_location": download_location
            },
            "description": "A mountain at sunrise",
            "alt_description": "mountain landscape",
            "user": { "name": "Ada Lovelace" }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/photos/abc123/download"))
        .and(header("Authorization", "Client-ID key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "url": image_url
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/images/abc123.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"jpeg"))
        .mount(&server)
        .await;

    std::env::set_var("UNSPLASH_API_BASE", server.uri());

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
        "sources": [
            { "enabled": true, "type": "unsplash", "query": "mountains" }
        ],
    });
    fs::write(
        root.path().join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(
        root.path().join("secrets.json"),
        r#"{"unsplash_access_key":"key"}"#,
    )
    .unwrap();
    fs::write(
        root.path().join("state.json"),
        r#"{"cache_queue":["unsplash:abc123"]}"#,
    )
    .unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx.advance_next().await.unwrap().expect("applied");
    let current = ctx.state.current.as_ref().expect("current");

    assert!(applied.ends_with("unsplash-abc123.jpg"));
    assert!(ctx.state.cache_queue.is_empty());
    assert_eq!(current.provider.as_deref(), Some("unsplash"));
    assert_eq!(
        current.source_url.as_deref(),
        Some("https://unsplash.com/photos/abc123")
    );
    assert_eq!(current.author.as_deref(), Some("Ada Lovelace"));
    assert_eq!(
        current.description.as_deref(),
        Some("A mountain at sunrise")
    );
    assert!(ctx
        .provider_status_report
        .attempts
        .iter()
        .any(|attempt| attempt.provider_id == "unsplash"
            && attempt.operation == ProviderOperation::AdvanceNext));
}
