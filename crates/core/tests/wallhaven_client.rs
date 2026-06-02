use walls_core::config::WallhavenSearch;
use walls_core::wallhaven::WallhavenClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn search_sends_api_key_and_parses_response() {
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