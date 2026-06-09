//! End-to-end proof that each implemented wallpaper source can fetch and apply.
//!
//! Hermetic tests use wiremock or local fixtures so CI stays offline-safe.
//! Live network smoke tests remain in `advance_next.rs` (`#[ignore]`).

mod common {
    include!("common/harness.rs");
    include!("common/wallhaven_mock.rs");
}

use std::fs;

use common::FetchHarness;
use serde_json::json;
use walls_core::providers::{configured_source_providers, ProviderKind};
use walls_core::WallsCtx;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn advance_expect_applied(mut ctx: WallsCtx) -> std::path::PathBuf {
    let applied = ctx
        .advance_next()
        .await
        .unwrap()
        .expect("advance_next should apply a wallpaper");
    assert!(
        applied.exists(),
        "applied path should exist: {}",
        applied.display()
    );
    assert!(ctx.state.current.is_some(), "state.current should be set");
    assert_eq!(ctx.state.history.len(), 1);
    applied
}

// --- Local sources (no network) ---

#[tokio::test]
async fn e2e_folder_source_fetches_local_image() {
    let harness = FetchHarness::new();
    let image = common::write_image(harness.path(), "images/wall.jpg", b"fake jpeg");
    harness.write_config(harness.base_config(
        false,
        json!([{ "enabled": true, "type": "folder", "path": image.parent().unwrap().display().to_string() }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("wall.jpg"));
}

#[tokio::test]
async fn e2e_image_source_fetches_single_file() {
    let harness = FetchHarness::new();
    let image = common::write_image(harness.path(), "single/wall.png", b"fake png");
    harness.write_config(harness.base_config(
        false,
        json!([{ "enabled": true, "type": "image", "path": image.display().to_string() }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("wall.png"));
}

#[tokio::test]
async fn e2e_favorites_source_fetches_from_favorites_dir() {
    let harness = FetchHarness::new();
    let image = common::write_image(harness.path(), "favorites/fav.jpg", b"fav");
    harness.write_config(harness.base_config(
        false,
        json!([{ "enabled": true, "type": "favorites", "label": "Favorites" }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert_eq!(applied, image);
}

#[tokio::test]
async fn e2e_fetched_source_fetches_from_fetched_dir() {
    let harness = FetchHarness::new();
    let image = common::write_image(harness.path(), "fetched/import.jpg", b"fetch");
    harness.write_config(harness.base_config(
        false,
        json!([{ "enabled": true, "type": "fetched", "label": "Fetched" }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert_eq!(applied, image);
}

// --- Online sources (wiremock) ---

#[tokio::test]
async fn e2e_bing_source_fetches_via_mock_archive() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/HPImageArchive.aspx"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "images": [{ "url": "/test-bing.jpg" }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/test-bing.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bing-jpeg"))
        .mount(&server)
        .await;
    std::env::set_var("BING_API_BASE", server.uri());

    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        true,
        json!([{ "enabled": true, "type": "bing", "label": "Bing daily" }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("bing-daily.jpg"));
    assert!(fs::read(&applied).unwrap().starts_with(b"bing"));
}

#[tokio::test]
async fn e2e_json_source_fetches_via_mock_feed() {
    let server = MockServer::start().await;
    let image_url = format!("{}/image.jpg", server.uri());
    Mock::given(method("GET"))
        .and(path("/feed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "download_url": image_url
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/image.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"json-jpeg"))
        .mount(&server)
        .await;

    let harness = FetchHarness::new();
    let feed_url = format!("{}/feed", server.uri());
    harness.write_config(harness.base_config(
        true,
        json!([{
            "enabled": true,
            "type": "json",
            "url": feed_url,
            "image_path": "$.download_url",
            "label": "Demo JSON"
        }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("json-feed.jpg"));
}

#[tokio::test]
async fn e2e_mediarss_source_fetches_via_mock_rss() {
    let server = MockServer::start().await;
    let image_url = format!("{}/rss-image.jpg", server.uri());
    let rss = format!(
        r#"<?xml version="1.0"?><rss><channel><item>
        <enclosure url="{image_url}" type="image/jpeg"/>
        </item></channel></rss>"#
    );
    Mock::given(method("GET"))
        .and(path("/rss"))
        .respond_with(ResponseTemplate::new(200).set_body_string(rss))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rss-image.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"rss-jpeg"))
        .mount(&server)
        .await;

    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        true,
        json!([{
            "enabled": true,
            "type": "mediarss",
            "url": format!("{}/rss", server.uri()),
            "label": "NASA RSS"
        }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("mediarss.jpg"));
}

#[tokio::test]
async fn e2e_wallhaven_source_refills_and_downloads_via_mock() {
    let server = MockServer::start().await;
    let _api_base = common::mount_wallhaven_fetch_flow(&server, "test-key").await;

    let harness = FetchHarness::new();
    harness
        .write_config(harness.wallhaven_only_config(FetchHarness::wallhaven_provider(json!({}))));
    harness.write_secrets(FetchHarness::wallhaven_secrets("test-key"));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("wallhaven-94x38z.jpg"));
}

#[tokio::test]
async fn e2e_wallhaven_refills_without_api_key_via_mock() {
    let server = MockServer::start().await;
    let _api_base = common::mount_wallhaven_fetch_flow(&server, "").await;

    let harness = FetchHarness::new();
    harness
        .write_config(harness.wallhaven_only_config(FetchHarness::wallhaven_provider(json!({}))));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("wallhaven-94x38z.jpg"));
}

#[tokio::test]
async fn e2e_wallhaven_applies_preseeded_cached_file_without_network() {
    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        false,
        json!([{ "enabled": true, "type": "wallhaven", "query": "space" }]),
    ));
    harness.write_secrets(json!({}));
    harness.write_cache_file("wallhaven-94x38z.jpg", b"jpeg");
    harness.write_state(json!({
        "cache_queue": ["94x38z"],
        "history": [],
    }));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("wallhaven-94x38z.jpg"));
}

#[tokio::test]
async fn e2e_sequential_sources_apply_local_before_queued_wallhaven() {
    let harness = FetchHarness::new();
    let favorite = common::write_image(harness.path(), "favorites/fav.jpg", b"fav");
    harness.write_cache_file("wallhaven-94x38z.jpg", b"jpeg");
    harness.write_config(harness.base_config(
        false,
        json!([
            { "enabled": true, "type": "favorites", "label": "Favorites" },
            { "enabled": true, "type": "wallhaven", "query": "space" }
        ]),
    ));
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(harness.path().join("config.json")).unwrap())
            .unwrap();
    config["selection"]["strategy"] = json!("sequential");
    harness.write_config(config);
    harness.write_secrets(json!({}));
    harness.write_state(json!({
        "cache_queue": ["94x38z"],
        "history": [],
    }));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert_eq!(applied, favorite);
}

#[tokio::test]
async fn e2e_advance_falls_through_when_wallhaven_refill_fails() {
    let server = MockServer::start().await;
    let _api_base = common::lock_wallhaven_api_base(&server.uri());
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({ "error": "Unauthorized" })))
        .mount(&server)
        .await;

    let harness = FetchHarness::new();
    let image = common::write_image(harness.path(), "images/fallback.jpg", b"fake jpeg");
    harness.write_config(harness.base_config_with_wallhaven(
        true,
        json!([
            { "enabled": true, "type": "wallhaven", "query": "nature" },
            {
                "enabled": true,
                "type": "folder",
                "path": image.parent().unwrap().display().to_string()
            }
        ]),
        FetchHarness::wallhaven_provider(json!({})),
    ));
    harness.write_secrets(FetchHarness::wallhaven_secrets("bad-key"));
    harness.write_state(json!({ "cache_queue": [], "history": [] }));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("fallback.jpg"));
}

#[tokio::test]
async fn e2e_unsplash_source_refills_and_downloads_via_mock() {
    let server = MockServer::start().await;
    let image_url = format!("{}/images/abc123.jpg", server.uri());
    let download_location = format!("{}/photos/abc123/download", server.uri());

    Mock::given(method("GET"))
        .and(path("/photos/random"))
        .and(header("Authorization", "Client-ID unsplash-key"))
        .and(query_param("query", "forest"))
        .and(query_param("content_filter", "high"))
        .and(query_param("count", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/unsplash-random.json"),
            "application/json",
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/photos/abc123"))
        .and(header("Authorization", "Client-ID unsplash-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "abc123",
            "urls": { "raw": image_url, "full": image_url, "regular": image_url },
            "links": {
                "html": "https://unsplash.com/photos/abc123",
                "download_location": download_location
            },
            "description": "Forest trail",
            "user": { "name": "Test Photographer" }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/photos/abc123/download"))
        .and(header("Authorization", "Client-ID unsplash-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "url": image_url })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/images/abc123.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"unsplash-jpeg"))
        .mount(&server)
        .await;
    std::env::set_var("UNSPLASH_API_BASE", server.uri());

    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        true,
        json!([{ "enabled": true, "type": "unsplash", "query": "forest" }]),
    ));
    harness.write_secrets(json!({ "unsplash_access_key": "unsplash-key" }));

    let mut ctx = harness.load_ctx();
    let applied = ctx
        .advance_next()
        .await
        .unwrap()
        .expect("unsplash should apply");
    assert!(applied.ends_with("unsplash-abc123.jpg"));
    assert_eq!(
        ctx.state.current.as_ref().unwrap().provider.as_deref(),
        Some("unsplash")
    );
}

// --- Newly wired inline providers ---

#[tokio::test]
async fn e2e_reddit_source_fetches_via_mock_listing() {
    let server = MockServer::start().await;
    let image_url = format!("{}/reddit-wall.jpg", server.uri());
    let listing = serde_json::json!({
        "data": {
            "children": [{
                "data": {
                    "title": "Test wallpaper",
                    "author": "alice",
                    "permalink": "/r/wallpapers/comments/abc123/test/",
                    "over_18": false,
                    "url": image_url
                }
            }]
        }
    });
    Mock::given(method("GET"))
        .and(path("/r/wallpapers/.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(listing))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/reddit-wall.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"reddit-jpeg"))
        .mount(&server)
        .await;
    std::env::set_var("REDDIT_API_BASE", server.uri());

    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        true,
        json!([{ "enabled": true, "type": "reddit", "query": "wallpapers", "sort": "hot" }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("reddit-fetch.jpg"));
}

#[tokio::test]
async fn e2e_apod_source_fetches_via_mock_api() {
    let server = MockServer::start().await;
    let image_url = format!("{}/apod.jpg", server.uri());
    Mock::given(method("GET"))
        .and(path("/planetary/apod"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "title": "Pillars of Creation",
            "url": image_url,
            "media_type": "image"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/apod.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"apod-jpeg"))
        .mount(&server)
        .await;
    std::env::set_var("NASA_API_BASE", server.uri());

    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        true,
        json!([{ "enabled": true, "type": "apod", "label": "NASA APOD" }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("apod-daily.jpg"));
}

#[tokio::test]
async fn e2e_pixabay_source_fetches_via_mock_api() {
    let server = MockServer::start().await;
    let image_url = format!("{}/pixabay.jpg", server.uri());
    Mock::given(method("GET"))
        .and(path("/api/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hits": [{ "largeImageURL": image_url, "pageURL": "https://pixabay.com/photo/1", "user": "bob", "tags": "forest" }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pixabay.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"pixabay-jpeg"))
        .mount(&server)
        .await;
    std::env::set_var("PIXABAY_API_BASE", server.uri());

    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        true,
        json!([{ "enabled": true, "type": "pixabay", "query": "nature", "api_key": "demo-key" }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("pixabay-fetch.jpg"));
}

#[tokio::test]
async fn e2e_immich_source_fetches_via_mock_api() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/search/random"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "asset-42", "originalFileName": "holiday.jpg" }
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/assets/asset-42/original"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"immich-jpeg"))
        .mount(&server)
        .await;

    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        true,
        json!([{
            "enabled": true,
            "type": "immich",
            "url": server.uri(),
            "api_key": "immich-key"
        }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("immich-fetch.jpg"));
}

#[tokio::test]
async fn e2e_attribution_source_fetches_direct_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/wall.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"attr-jpeg"))
        .mount(&server)
        .await;

    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        true,
        json!([{
            "enabled": true,
            "type": "attribution",
            "label": "Example",
            "url": format!("{}/wall.jpg", server.uri())
        }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("attribution-fetch.jpg"));
}

#[tokio::test]
async fn e2e_spotlight_source_picks_from_configured_folder() {
    let harness = FetchHarness::new();
    let image = common::write_image(harness.path(), "spotlight-cache/wall.jpg", b"spotlight");
    harness.write_config(harness.base_config(
        false,
        json!([{
            "enabled": true,
            "type": "spotlight",
            "label": "Spotlight",
            "path": image.parent().unwrap().display().to_string()
        }]),
    ));
    harness.write_secrets(json!({}));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert_eq!(applied, image);
}

// Weighting is a selection modifier, not a fetch source.
#[tokio::test]
async fn e2e_weighting_source_does_not_fetch_by_itself() {
    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(
        true,
        json!([{ "enabled": true, "type": "weighting", "query": "high" }]),
    ));
    harness.write_secrets(json!({}));

    let providers = configured_source_providers(&harness.load_ctx().config.sources);
    assert_eq!(providers[0].kind, ProviderKind::Weighting);

    let mut ctx = harness.load_ctx();
    let applied = ctx.advance_next().await.unwrap();
    assert!(
        applied.is_none(),
        "weighting should not fetch wallpapers by itself"
    );
}
