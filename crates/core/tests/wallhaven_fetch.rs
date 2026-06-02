use walls_core::wallhaven::WallhavenClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetch_wallpaper_parses_single_wallpaper() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/w/94x38z"))
        .and(header("X-API-Key", "key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/wallhaven-wallpaper.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "key").unwrap();
    let wp = client.fetch_wallpaper("94x38z").await.unwrap();
    assert_eq!(wp.id, "94x38z");
    assert!(wp.path.contains("94x38z"));
}
