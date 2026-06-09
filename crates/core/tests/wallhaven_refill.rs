use walls_core::config::{
    default_wallhaven_source, ChangeConfig, Config, PathsConfig, SelectionConfig, SourceEntry,
};
use walls_core::state::State;
use walls_core::wallhaven::{refill_wallhaven_cache, WallhavenClient};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config() -> Config {
    Config {
        change: ChangeConfig {
            internet_enabled: true,
            ..ChangeConfig::default()
        },
        paths: PathsConfig {
            cache_dir: "~/.cache".into(),
            download_dir: "~/.download".into(),
            favorites_dir: "~/.fav".into(),
            fetched_dir: "~/.fetch".into(),
            compose_dir: "~/.compose".into(),
        },
        quota: Default::default(),
        apply: Default::default(),
        display: Default::default(),
        selection: SelectionConfig {
            refetch_when_cache_below: 5,
            ..SelectionConfig::default()
        },
        sources: vec![default_wallhaven_source()],
        tray: Default::default(),
        tui: Default::default(),
    }
}

#[tokio::test]
async fn refill_pushes_search_ids_into_cache_queue() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/wallhaven-search.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "key").unwrap();
    let config = test_config();
    let mut state = State::default();

    refill_wallhaven_cache(&client, &config, &mut state)
        .await
        .unwrap();

    assert_eq!(state.cache_queue, vec!["94x38z"]);
}

#[tokio::test]
async fn refill_uses_wallhaven_source_queries() {
    let server = MockServer::start().await;
    for query in ["jupiter", "neptune"] {
        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .and(header("X-API-Key", "key"))
            .and(query_param("q", query))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                include_str!("fixtures/wallhaven-search.json"),
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;
    }

    let client = WallhavenClient::new(server.uri(), "key").unwrap();
    let mut config = test_config();
    config.sources = vec![
        SourceEntry {
            enabled: true,
            source_type: "wallhaven".into(),
            query: Some("jupiter".into()),
            ..default_wallhaven_source()
        },
        SourceEntry {
            enabled: true,
            source_type: "wallhaven".into(),
            query: Some("neptune".into()),
            ..default_wallhaven_source()
        },
    ];
    let mut state = State::default();

    refill_wallhaven_cache(&client, &config, &mut state)
        .await
        .unwrap();

    assert_eq!(state.cache_queue, vec!["94x38z"]);
    assert_eq!(
        state.wallhaven.source_search_pages.get("0:jupiter"),
        Some(&2)
    );
    assert_eq!(
        state.wallhaven.source_search_pages.get("1:neptune"),
        Some(&2)
    );
}
