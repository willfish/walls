use walls_core::config::{ChangeConfig, Config, PathsConfig, SelectionConfig};
use walls_core::state::State;
use walls_core::unsplash::{refill_unsplash_cache, UnsplashClient};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config() -> Config {
    serde_json::from_value(serde_json::json!({
        "change": { "internet_enabled": true },
        "paths": {
            "cache_dir": "~/.cache",
            "download_dir": "~/.download",
            "favorites_dir": "~/.fav",
            "fetched_dir": "~/.fetch",
            "compose_dir": "~/.compose"
        },
        "selection": { "refetch_when_cache_below": 5 },
        "sources": [
            {
                "enabled": true,
                "type": "unsplash",
                "label": "Nature",
                "query": "forest",
                "orientation": "landscape"
            }
        ]
    }))
    .expect("config")
}

#[tokio::test]
async fn refill_pushes_unsplash_ids_into_prefixed_cache_queue() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/photos/random"))
        .and(header("Authorization", "Client-ID key"))
        .and(query_param("query", "forest"))
        .and(query_param("orientation", "landscape"))
        .and(query_param("content_filter", "high"))
        .and(query_param("count", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/unsplash-random.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let client = UnsplashClient::new(server.uri(), "key").unwrap();
    let config = test_config();
    let mut state = State::default();

    refill_unsplash_cache(&client, &config, &mut state)
        .await
        .unwrap();

    assert_eq!(state.cache_queue, vec!["unsplash:abc123"]);
}

#[test]
fn config_defaults_keep_existing_manual_construction_shape() {
    let config = Config {
        change: ChangeConfig::default(),
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
        selection: SelectionConfig::default(),
        sources: vec![],
        wallhaven: Default::default(),
        tray: Default::default(),
    };

    assert!(config.sources.is_empty());
}
