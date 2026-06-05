use walls_core::config::{Config, SourceEntry};
use walls_core::providers::{
    configured_providers, configured_source_providers, enabled_local_sources, unsplash_provider,
    wallhaven_provider, ProviderCapability, ProviderKind,
};

fn test_config(internet_enabled: bool) -> Config {
    serde_json::from_value(serde_json::json!({
        "change": { "internet_enabled": internet_enabled },
        "paths": {
            "cache_dir": "cache",
            "download_dir": "downloaded",
            "favorites_dir": "favorites",
            "fetched_dir": "fetched",
            "compose_dir": "wallpaper"
        },
        "sources": []
    }))
    .expect("config")
}

fn test_config_with_sources(internet_enabled: bool, sources: serde_json::Value) -> Config {
    let mut config = test_config(internet_enabled);
    config.sources = serde_json::from_value(sources).expect("sources");
    config
}

#[test]
fn classifies_existing_source_entries_without_schema_changes() {
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "folder", "label": "Wallpapers", "path": "/tmp/walls" },
        { "enabled": true, "type": "favorites" },
        { "enabled": false, "type": "image", "path": "/tmp/wall.jpg" },
        { "enabled": true, "type": "unsplash", "query": "forest" },
        { "enabled": true, "type": "future-provider", "url": "https://example.com/feed.json" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_eq!(providers[0].id, "Wallpapers");
    assert_eq!(providers[0].kind, ProviderKind::Local);
    assert!(providers[0].enabled);
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::ConfigValidation));
    assert!(!providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert_eq!(providers[1].id, "favorites");
    assert_eq!(providers[1].kind, ProviderKind::Local);
    assert_eq!(providers[2].kind, ProviderKind::Local);
    assert!(!providers[2].enabled);
    assert_eq!(providers[3].kind, ProviderKind::Unsplash);
    assert!(providers[3]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert_eq!(providers[4].kind, ProviderKind::Unsupported);
    assert!(!providers[4]
        .capabilities
        .contains(&ProviderCapability::ConfigValidation));
}

#[test]
fn configured_providers_include_sources_and_wallhaven_adapter() {
    let config = test_config_with_sources(
        true,
        serde_json::json!([
            { "enabled": true, "type": "folder", "label": "Local", "path": "/tmp/walls" }
        ]),
    );
    let secrets = serde_json::from_value::<walls_core::config::Secrets>(serde_json::json!({
        "wallhaven_api_key": "key"
    }))
    .expect("secrets");

    let providers = configured_providers(&config, &secrets);

    assert_eq!(providers.len(), 2);
    assert_eq!(providers[0].id, "Local");
    assert_eq!(providers[0].kind, ProviderKind::Local);
    assert_eq!(providers[1].id, "wallhaven");
    assert_eq!(providers[1].kind, ProviderKind::Wallhaven);
    assert!(providers[1].enabled);
}

#[test]
fn enabled_local_sources_dispatch_excludes_disabled_and_unsupported_sources() {
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "folder", "path": "/tmp/walls" },
        { "enabled": false, "type": "favorites" },
        { "enabled": true, "type": "future-provider", "url": "https://example.com/feed.json" },
        { "enabled": true, "type": "fetched" }
    ]))
    .expect("sources");

    let dispatched: Vec<&str> = enabled_local_sources(&sources)
        .map(|source| source.source_type.as_str())
        .collect();

    assert_eq!(dispatched, vec!["folder", "fetched"]);
}

#[test]
fn wallhaven_descriptor_preserves_existing_enablement_rules() {
    let mut secrets = serde_json::from_value::<walls_core::config::Secrets>(serde_json::json!({
        "wallhaven_api_key": "key"
    }))
    .expect("secrets");

    let provider = wallhaven_provider(&test_config(true), &secrets);
    assert!(provider.enabled);
    assert!(provider
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(provider
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(provider
        .capabilities
        .contains(&ProviderCapability::Metadata));
    assert!(!wallhaven_provider(&test_config(false), &secrets).enabled);

    secrets.wallhaven_api_key.clear();
    assert!(!wallhaven_provider(&test_config(true), &secrets).enabled);
}

#[test]
fn unsplash_descriptor_preserves_enablement_rules() {
    let config = test_config_with_sources(
        true,
        serde_json::json!([
            { "enabled": true, "type": "unsplash", "query": "forest" }
        ]),
    );
    let secrets = serde_json::from_value::<walls_core::config::Secrets>(serde_json::json!({
        "unsplash_access_key": "key"
    }))
    .expect("secrets");

    let provider = unsplash_provider(&config, &secrets);

    assert_eq!(provider.kind, ProviderKind::Unsplash);
    assert!(provider.enabled);
    assert!(provider
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(provider
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(
        !unsplash_provider(
            &test_config_with_sources(false, serde_json::json!([])),
            &secrets
        )
        .enabled
    );
}

#[test]
fn failure_scope_names_provider_and_operation() {
    let provider = configured_source_providers(&[SourceEntry {
        enabled: true,
        source_type: "folder".into(),
        label: Some("Local library".into()),
        path: Some("/tmp/walls".into()),
        query: None,
        url: None,
        collection: None,
        user: None,
        topic: None,
        orientation: None,
    }])
    .remove(0);

    let scope = provider.failure_scope("local source listing").to_string();

    assert!(scope.contains("Local library"), "{scope}");
    assert!(scope.contains("Local"), "{scope}");
    assert!(scope.contains("local source listing"), "{scope}");
}

#[test]
fn reddit_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED phase for #158: configurable Reddit source (subreddit via query for now).
    // Currently "reddit" type -> Unsupported (no Download/Refill caps).
    // Test must fail until ProviderKind::Reddit + source_kind + caps wiring added (minimal per AC).
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "reddit", "query": "wallpapers" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "reddit must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}

#[test]
fn bing_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED for #159 (will fail until Bing kind added).
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "bing", "query": "daily" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "bing must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}

#[test]
fn apod_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED for #160 (will fail until Apod kind added).
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "apod", "query": "daily" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "apod must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}

#[test]
fn mediarss_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED for #161 (will fail until MediaRss kind added).
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "mediarss", "url": "https://example.com/feed.xml" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "mediarss must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}

#[test]
fn attribution_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED for #162 (will fail until Attribution kind added).
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "attribution", "query": "meta" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "attribution must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}

#[test]
fn jsonfeed_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED for #163 generic JSON image feed (will fail until Json kind added).
    // Per AC: supports URL + JSON path selectors.
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "json", "url": "https://example.com/feed.json" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "json must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}

#[test]
fn pixabay_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED for #164 Pixabay (will fail until Pixabay kind).
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "pixabay", "query": "nature" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "pixabay must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}

#[test]
fn immich_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED for #165 Immich (will fail until Immich kind).
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "immich", "url": "https://immich.example.com" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "immich must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}

#[test]
fn spotlight_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED for #166 Windows Spotlight evaluate (will fail until Spotlight kind).
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "spotlight", "url": "https://example.com" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "spotlight must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}

#[test]
fn weighting_source_is_classified_with_full_capabilities_not_unsupported() {
    // RED for #167 per-source weighting (will fail until Weighting kind).
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "weighting", "query": "high" }
    ]))
    .expect("sources");

    let providers = configured_source_providers(&sources);

    assert_ne!(
        providers[0].kind,
        ProviderKind::Unsupported,
        "weighting must not be Unsupported"
    );
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Download));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::QueueRefill));
    assert!(providers[0]
        .capabilities
        .contains(&ProviderCapability::Metadata));
}
