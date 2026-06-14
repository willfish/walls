use walls_core::config::{
    default_wallhaven_source, ChangeConfig, Config, PathsConfig, SelectionConfig, SourceEntry,
};
use walls_core::state::State;
use walls_core::wallhaven::{refill_wallhaven_cache, WallhavenClient};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

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

#[tokio::test]
async fn refill_allows_empty_wallhaven_source_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "key"))
        .respond_with(|request: &Request| {
            let query = request.url.query().unwrap_or_default();
            assert!(query.contains("q="), "{query}");
            assert!(!query.contains("q=space"), "{query}");
            assert!(query.contains("ratios=3x2"), "{query}");
            assert!(query.contains("atleast=2560x1440"), "{query}");
            ResponseTemplate::new(200).set_body_raw(
                include_str!("fixtures/wallhaven-search.json"),
                "application/json",
            )
        })
        .expect(1)
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "key").unwrap();
    let mut config = test_config();
    config.sources[0].query = Some("   ".into());
    config.sources[0].label = Some("Filters only".into());
    config.sources[0].ratios = Some("3x2".into());
    config.sources[0].atleast = Some("2560x1440".into());
    let mut state = State::default();

    refill_wallhaven_cache(&client, &config, &mut state)
        .await
        .unwrap();

    assert_eq!(state.cache_queue, vec!["94x38z"]);
    assert_eq!(
        state.wallhaven.source_search_pages.get("0:Filters only"),
        Some(&2)
    );
}

#[tokio::test]
async fn refill_drops_ratio_and_resolution_for_empty_query_fallback() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "key"))
        .and(query_param("q", ""))
        .and(query_param("ratios", "3x2"))
        .and(query_param("atleast", "2560x1440"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"data":[],"meta":{"current_page":1,"last_page":1}}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "key"))
        .and(query_param("q", ""))
        .respond_with(|request: &Request| {
            let query = request.url.query().unwrap_or_default();
            assert!(!query.contains("ratios="), "{query}");
            assert!(!query.contains("atleast="), "{query}");
            ResponseTemplate::new(200).set_body_raw(
                include_str!("fixtures/wallhaven-search.json"),
                "application/json",
            )
        })
        .expect(1)
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "key").unwrap();
    let mut config = test_config();
    config.selection.refetch_when_cache_below = 1;
    config.sources[0].query = None;
    config.sources[0].ratios = Some("3x2".into());
    config.sources[0].atleast = Some("2560x1440".into());
    let mut state = State::default();

    refill_wallhaven_cache(&client, &config, &mut state)
        .await
        .unwrap();

    assert_eq!(state.cache_queue, vec!["94x38z"]);
    assert_eq!(
        state.wallhaven.source_search_pages.get("0:<blank>"),
        Some(&1)
    );
}

#[tokio::test]
async fn refill_drops_ratio_and_resolution_when_exact_search_is_exhausted() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "key"))
        .and(query_param("ratios", "3x2"))
        .and(query_param("atleast", "2560x1440"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"data":[],"meta":{"current_page":1,"last_page":1}}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "key"))
        .respond_with(|request: &Request| {
            let query = request.url.query().unwrap_or_default();
            assert!(!query.contains("ratios="), "{query}");
            assert!(!query.contains("atleast="), "{query}");
            ResponseTemplate::new(200).set_body_raw(
                include_str!("fixtures/wallhaven-search.json"),
                "application/json",
            )
        })
        .expect(1)
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "key").unwrap();
    let mut config = test_config();
    config.selection.refetch_when_cache_below = 1;
    config.sources[0].ratios = Some("3x2".into());
    config.sources[0].atleast = Some("2560x1440".into());
    let mut state = State::default();

    refill_wallhaven_cache(&client, &config, &mut state)
        .await
        .unwrap();

    assert_eq!(state.cache_queue, vec!["94x38z"]);
    assert_eq!(state.wallhaven.source_search_pages.get("0:space"), Some(&1));
}

#[tokio::test]
async fn refill_broadens_when_source_cache_threshold_is_not_met() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "key"))
        .and(query_param("ratios", "3x2"))
        .and(query_param("atleast", "2560x1440"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"data":[{"id":"exact-one","path":"https://w.wallhaven.cc/full/ex/wallhaven-exact-one.jpg"}],"meta":{"current_page":1,"last_page":36}}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-API-Key", "key"))
        .respond_with(|request: &Request| {
            let query = request.url.query().unwrap_or_default();
            assert!(!query.contains("ratios="), "{query}");
            assert!(!query.contains("atleast="), "{query}");
            ResponseTemplate::new(200).set_body_raw(
                r#"{"data":[{"id":"broad-one","path":"https://w.wallhaven.cc/full/br/wallhaven-broad-one.jpg"}],"meta":{"current_page":1,"last_page":36}}"#,
                "application/json",
            )
        })
        .expect(1)
        .mount(&server)
        .await;

    let client = WallhavenClient::new(server.uri(), "key").unwrap();
    let mut config = test_config();
    config.selection.refetch_when_cache_below = 5;
    config.sources[0].ratios = Some("3x2".into());
    config.sources[0].atleast = Some("2560x1440".into());
    config.sources[0].broaden_when_cache_below = Some(2);
    let mut state = State::default();

    refill_wallhaven_cache(&client, &config, &mut state)
        .await
        .unwrap();

    assert_eq!(state.cache_queue, vec!["exact-one", "broad-one"]);
    assert_eq!(state.wallhaven.source_search_pages.get("0:space"), Some(&2));
    let effective = state
        .wallhaven
        .effective_source_searches
        .get("0:space")
        .expect("effective source search");
    assert_eq!(effective.q, "space");
    assert_eq!(effective.ratios, "");
    assert_eq!(effective.atleast, "");
}
