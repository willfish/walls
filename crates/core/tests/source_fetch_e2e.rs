//! End-to-end proof that each implemented wallpaper source can fetch and apply.
//!
//! Hermetic tests use wiremock or local fixtures so CI stays offline-safe.
//! Live network smoke tests remain in `advance_next.rs` (`#[ignore]`).

mod common;

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
    let image_url = format!("{}/wallhaven-94x38z.jpg", server.uri());

    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/wallhaven-search.json"),
            "application/json",
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/w/94x38z"))
        .and(header("X-API-Key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "id": "94x38z", "path": image_url }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/wallhaven-94x38z.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"wallhaven-jpeg"))
        .mount(&server)
        .await;
    std::env::set_var("WALLHAVEN_API_BASE", server.uri());

    let harness = FetchHarness::new();
    harness.write_config(json!({
        "change": { "enabled": true, "internet_enabled": true },
        "paths": common::paths_block(harness.path()),
        "apply": common::apply_block(&harness.noop),
        "display": { "mode": "os" },
        "selection": { "refetch_when_cache_below": 5 },
        "sources": [],
        "wallhaven": {
            "enabled": true,
            "prefer": "search_only",
            "search": { "q": "nature", "purity": "100" }
        }
    }));
    harness.write_secrets(json!({ "wallhaven_api_key": "test-key" }));

    let applied = advance_expect_applied(harness.load_ctx()).await;
    assert!(applied.ends_with("wallhaven-94x38z.jpg"));
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

// --- Config-only sources (classified but fetch not wired yet) ---

#[tokio::test]
async fn e2e_reddit_source_is_classified_but_not_fetched_yet() {
    assert_unimplemented_online_source(
        "reddit",
        json!({ "enabled": true, "type": "reddit", "query": "wallpapers", "sort": "hot" }),
        ProviderKind::Reddit,
    )
    .await;
}

#[tokio::test]
async fn e2e_apod_source_is_classified_but_not_fetched_yet() {
    assert_unimplemented_online_source(
        "apod",
        json!({ "enabled": true, "type": "apod", "label": "NASA APOD" }),
        ProviderKind::Apod,
    )
    .await;
}

#[tokio::test]
async fn e2e_pixabay_source_is_classified_but_not_fetched_yet() {
    assert_unimplemented_online_source(
        "pixabay",
        json!({ "enabled": true, "type": "pixabay", "query": "nature", "api_key": "demo" }),
        ProviderKind::Pixabay,
    )
    .await;
}

#[tokio::test]
async fn e2e_immich_source_is_classified_but_not_fetched_yet() {
    assert_unimplemented_online_source(
        "immich",
        json!({
            "enabled": true,
            "type": "immich",
            "url": "https://immich.example.com",
            "api_key": "demo"
        }),
        ProviderKind::Immich,
    )
    .await;
}

#[tokio::test]
async fn e2e_attribution_source_is_classified_but_not_fetched_yet() {
    assert_unimplemented_online_source(
        "attribution",
        json!({
            "enabled": true,
            "type": "attribution",
            "url": "https://example.com/wall.jpg"
        }),
        ProviderKind::Attribution,
    )
    .await;
}

#[tokio::test]
async fn e2e_spotlight_source_is_classified_but_not_fetched_yet() {
    assert_unimplemented_online_source(
        "spotlight",
        json!({ "enabled": true, "type": "spotlight", "label": "Spotlight" }),
        ProviderKind::Spotlight,
    )
    .await;
}

#[tokio::test]
async fn e2e_weighting_source_is_classified_but_not_fetched_yet() {
    assert_unimplemented_online_source(
        "weighting",
        json!({ "enabled": true, "type": "weighting", "query": "high" }),
        ProviderKind::Weighting,
    )
    .await;
}

async fn assert_unimplemented_online_source(
    label: &str,
    source: serde_json::Value,
    expected_kind: ProviderKind,
) {
    let harness = FetchHarness::new();
    harness.write_config(harness.base_config(true, json!([source])));
    harness.write_secrets(json!({}));

    let ctx = harness.load_ctx();
    let providers = configured_source_providers(&ctx.config.sources);
    assert_eq!(providers.len(), 1, "{label} should have one provider");
    assert_eq!(providers[0].kind, expected_kind, "{label} kind");

    let mut ctx = ctx;
    let applied = ctx.advance_next().await.unwrap();
    assert!(
        applied.is_none(),
        "{label} fetch is not implemented yet; advance_next should return None"
    );
}
