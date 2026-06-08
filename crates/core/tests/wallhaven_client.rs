use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use walls_core::config::WallhavenSearch;
use walls_core::wallhaven::client::purity_for_request;
use walls_core::wallhaven::WallhavenClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

#[tokio::test]
async fn wallhaven_search_sends_api_key_and_parses_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/wallhaven-search.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "test-key").unwrap();
    let params = WallhavenSearch::default();
    let resp = client.search(&params, 1).await.unwrap();

    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].id, "94x38z");
}

#[tokio::test]
async fn wallhaven_search_retries_transient_server_error() {
    let server = MockServer::start().await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_responder = Arc::clone(&attempts);

    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "test-key"))
        .respond_with(move |_: &Request| {
            if attempts_for_responder.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(500)
            } else {
                ResponseTemplate::new(200).set_body_raw(
                    include_str!("fixtures/wallhaven-search.json"),
                    "application/json",
                )
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "test-key").unwrap();
    let params = WallhavenSearch::default();
    let resp = client.search(&params, 1).await.unwrap();

    assert_eq!(resp.data[0].id, "94x38z");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn wallhaven_search_stops_after_three_transient_attempts() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "test-key"))
        .respond_with(ResponseTemplate::new(503))
        .expect(3)
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "test-key").unwrap();
    let params = WallhavenSearch::default();
    let err = client.search(&params, 1).await.unwrap_err();

    let reqwest_error = err.downcast_ref::<reqwest::Error>().unwrap();
    assert_eq!(
        reqwest_error.status(),
        Some(reqwest::StatusCode::SERVICE_UNAVAILABLE)
    );
}

#[tokio::test]
async fn wallhaven_fetch_wallpaper_uses_request_timeout() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/w/94x38z"))
        .and(header("X-API-Key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(200))
                .set_body_raw(
                    include_str!("fixtures/wallhaven-wallpaper.json"),
                    "application/json",
                ),
        )
        .mount(&server)
        .await;

    let client = WallhavenClient::new_with_timeouts(
        server.uri(),
        "test-key",
        Duration::from_millis(20),
        Duration::from_millis(20),
    )
    .unwrap();
    let err = client.fetch_wallpaper("94x38z").await.unwrap_err();

    let reqwest_error = err.downcast_ref::<reqwest::Error>().unwrap();
    assert!(reqwest_error.is_timeout());
}

#[test]
fn purity_for_request_strips_nsfw_without_api_key() {
    assert_eq!(purity_for_request("111", ""), "110");
    assert_eq!(purity_for_request("101", ""), "100");
    assert_eq!(purity_for_request("111", "key"), "111");
}

#[tokio::test]
async fn wallhaven_search_works_without_api_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", ""))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/wallhaven-search.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "").unwrap();
    let params = WallhavenSearch {
        purity: "111".into(),
        ..Default::default()
    };
    let resp = client.search(&params, 1).await.unwrap();

    assert_eq!(resp.data[0].id, "94x38z");
}
