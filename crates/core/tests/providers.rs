use walls_core::config::{Config, SourceEntry};
use walls_core::providers::{
    configured_providers, configured_source_providers, enabled_local_sources, unsplash_provider,
    wallhaven_provider, ProviderAttemptOutcome, ProviderCapability, ProviderFailureKind,
    ProviderKind, ProviderNoCandidateReason, ProviderOperation, ProviderRetry, ProviderRetryReason,
    ProviderRunOutcome, ProviderStatus, ProviderStatusReport,
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
        // All the new classified providers from the v0.11 extensibility backlog.
        // These use the SourceEntry fields (query/url/user/collection/topic/orientation/label/api_key)
        // that match Variety's RedditSource, Bing, APOD, MediaRss, JSON, Pixabay, Immich, etc.
        // The test proves the public providers API (configured_source_providers, ProviderKind, caps)
        // accepts them without schema changes or falling to Unsupported.
        { "enabled": true, "type": "reddit", "query": "wallpapers", "label": "Reddit" },
        { "enabled": true, "type": "bing", "label": "Bing" },
        { "enabled": true, "type": "apod", "label": "APOD" },
        { "enabled": true, "type": "mediarss", "url": "https://example.com/rss", "label": "MediaRSS" },
        { "enabled": true, "type": "attribution", "url": "https://ex.com/img.jpg", "source": "NASA", "author": "Hubble", "label": "Attr" },
        { "enabled": true, "type": "json", "url": "https://ex.com/feed.json", "image_path": "$.img", "label": "JSON" },
        { "enabled": true, "type": "pixabay", "query": "cat", "label": "Pixabay" },
        { "enabled": true, "type": "immich", "url": "https://immich.ex", "api_key": "k", "label": "Immich" },
        { "enabled": true, "type": "spotlight", "label": "Spotlight" },
        { "enabled": true, "type": "weighting", "query": "high", "label": "Weighting" }
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

    // Feature the new providers: all classified, not Unsupported, full online caps.
    // This uses the public API and proves Variety-style configs for the 10 new kinds work.
    let new_kinds_start = 4;
    for (i, expected_kind) in [
        ProviderKind::Reddit,
        ProviderKind::Bing,
        ProviderKind::Apod,
        ProviderKind::MediaRss,
        ProviderKind::Attribution,
        ProviderKind::Json,
        ProviderKind::Pixabay,
        ProviderKind::Immich,
        ProviderKind::Spotlight,
        ProviderKind::Weighting,
    ]
    .iter()
    .enumerate()
    {
        let p = &providers[new_kinds_start + i];
        assert_eq!(
            p.kind, *expected_kind,
            "provider {} should be classified as {:?}",
            p.id, expected_kind
        );
        assert_ne!(p.kind, ProviderKind::Unsupported);
        assert!(p
            .capabilities
            .contains(&ProviderCapability::ConfigValidation));
        assert!(p.capabilities.contains(&ProviderCapability::QueueRefill));
        assert!(p.capabilities.contains(&ProviderCapability::Download));
        assert!(p.capabilities.contains(&ProviderCapability::Metadata));
    }
}

#[test]
fn configured_providers_include_sources_and_wallhaven_adapter() {
    let config = test_config_with_sources(
        true,
        serde_json::json!([
            { "enabled": true, "type": "folder", "label": "Local", "path": "/tmp/walls" },
            { "enabled": true, "type": "wallhaven", "query": "space" }
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
fn wallhaven_descriptor_requires_enabled_wallhaven_source() {
    let mut secrets = serde_json::from_value::<walls_core::config::Secrets>(serde_json::json!({
        "wallhaven_api_key": "key"
    }))
    .expect("secrets");

    let provider = wallhaven_provider(
        &test_config_with_sources(
            true,
            serde_json::json!([{ "enabled": true, "type": "wallhaven", "query": "space" }]),
        ),
        &secrets,
    );
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
    assert!(
        !wallhaven_provider(
            &test_config_with_sources(
                false,
                serde_json::json!([{ "enabled": true, "type": "wallhaven", "query": "space" }])
            ),
            &secrets
        )
        .enabled
    );

    secrets.wallhaven_api_key.clear();
    assert!(
        wallhaven_provider(
            &test_config_with_sources(
                true,
                serde_json::json!([{ "enabled": true, "type": "wallhaven", "query": "space" }])
            ),
            &secrets
        )
        .enabled
    );

    secrets.wallhaven_api_key = "key".into();
    let config = test_config(true);
    assert!(!wallhaven_provider(&config, &secrets).enabled);
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
        api_key: None,
        image_path: None,
        title_path: None,
        sort: None,
        time: None,
        ..SourceEntry::default()
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

// Features the public providers API (configured_providers, descriptors) with a realistic
// mix of the new classified kinds (using Variety-compatible SourceEntry fields).
// Proves that configs with reddit/bing/apod/... load into full-capability descriptors
// alongside wallhaven adapter, without breaking existing behavior.
#[test]
fn configured_providers_features_all_new_classified_kinds() {
    let config = test_config_with_sources(
        true,
        serde_json::json!([
            { "enabled": true, "type": "folder", "label": "Local", "path": "/tmp/walls" },
            { "enabled": true, "type": "reddit", "query": "wallpapers" },
            { "enabled": true, "type": "bing" },
            { "enabled": true, "type": "json", "url": "https://ex.com/feed.json", "image_path": "$.u" },
            { "enabled": true, "type": "wallhaven", "query": "space" }
        ]),
    );
    let secrets = serde_json::from_value::<walls_core::config::Secrets>(serde_json::json!({
        "wallhaven_api_key": "key",
        "unsplash_access_key": ""
    }))
    .expect("secrets");

    let providers = configured_providers(&config, &secrets);

    // sources (Local + 4 new) = 5
    assert_eq!(providers.len(), 5);
    assert_eq!(providers[0].kind, ProviderKind::Local);
    assert_eq!(providers[1].kind, ProviderKind::Reddit);
    assert_eq!(providers[2].kind, ProviderKind::Bing);
    assert_eq!(providers[3].kind, ProviderKind::Json);
    assert_eq!(providers[4].kind, ProviderKind::Wallhaven);

    // The new ones (1,2,3,4) have full caps (proves the classification API for extensibility)
    for p in [&providers[1], &providers[2], &providers[3], &providers[4]] {
        assert!(p.capabilities.contains(&ProviderCapability::Download));
        assert!(p.capabilities.contains(&ProviderCapability::Metadata));
    }
}

#[test]
fn provider_attempt_reports_offline_skip_with_fallback() {
    let provider = wallhaven_provider(&test_config(false), &walls_core::config::Secrets::default());

    let attempt = provider
        .attempt(ProviderOperation::QueueRefill)
        .with_status(ProviderStatus::OfflineDisabled)
        .skipped(ProviderNoCandidateReason::OfflineDisabled)
        .with_fallback("local");

    assert_eq!(attempt.provider_id, "wallhaven");
    assert_eq!(attempt.provider_kind, ProviderKind::Wallhaven);
    assert_eq!(attempt.status, ProviderStatus::OfflineDisabled);
    assert_eq!(
        attempt.outcome,
        ProviderAttemptOutcome::Skipped {
            reason: ProviderNoCandidateReason::OfflineDisabled
        }
    );
    assert_eq!(attempt.fallback_provider_id.as_deref(), Some("local"));
}

#[test]
fn provider_attempt_reports_missing_credentials() {
    let config = test_config_with_sources(
        true,
        serde_json::json!([
            { "enabled": true, "type": "unsplash", "query": "forest" }
        ]),
    );
    let provider = unsplash_provider(&config, &walls_core::config::Secrets::default());

    let attempt = provider
        .attempt(ProviderOperation::Search)
        .with_status(ProviderStatus::CredentialMissing)
        .skipped(ProviderNoCandidateReason::CredentialMissing);

    assert_eq!(provider.kind, ProviderKind::Unsplash);
    assert!(!provider.enabled);
    assert_eq!(attempt.status, ProviderStatus::CredentialMissing);
    assert_eq!(
        attempt.outcome,
        ProviderAttemptOutcome::Skipped {
            reason: ProviderNoCandidateReason::CredentialMissing
        }
    );
}

#[test]
fn provider_attempt_records_transient_rate_limit_retry() {
    let provider = wallhaven_provider(&test_config(true), &walls_core::config::Secrets::default());

    let attempt = provider
        .attempt(ProviderOperation::Search)
        .with_retry(ProviderRetry::rate_limited(1, 100))
        .applied(Some(12));

    assert_eq!(attempt.retries.len(), 1);
    assert_eq!(attempt.retries[0].attempt, 1);
    assert_eq!(attempt.retries[0].backoff_ms, 100);
    assert_eq!(attempt.retries[0].reason, ProviderRetryReason::RateLimited);
    assert_eq!(attempt.retries[0].status_code, Some(429));
    assert_eq!(
        attempt.outcome,
        ProviderAttemptOutcome::Applied {
            candidate_count: Some(12)
        }
    );
}

#[test]
fn provider_attempt_reports_terminal_failure_and_next_fallback() {
    let provider = wallhaven_provider(&test_config(true), &walls_core::config::Secrets::default());

    let attempt = provider
        .attempt(ProviderOperation::Download)
        .with_retry(ProviderRetry::server_error(1, 100, 503))
        .with_retry(ProviderRetry::server_error(2, 200, 503))
        .failed(
            ProviderFailureKind::Request,
            Some(503),
            Some("provider wallhaven failed during download".into()),
        )
        .with_fallback("bing");

    assert_eq!(attempt.retries.len(), 2);
    assert_eq!(attempt.fallback_provider_id.as_deref(), Some("bing"));
    assert_eq!(
        attempt.outcome,
        ProviderAttemptOutcome::Failed {
            kind: ProviderFailureKind::Request,
            status_code: Some(503),
            message: Some("provider wallhaven failed during download".into())
        }
    );
}

#[test]
fn provider_status_report_records_no_candidate_attempts() {
    let sources: Vec<SourceEntry> = serde_json::from_value(serde_json::json!([
        { "enabled": true, "type": "folder", "label": "Local", "path": "/tmp/walls" }
    ]))
    .expect("sources");
    let provider = configured_source_providers(&sources).remove(0);
    let mut report = ProviderStatusReport::default();

    report.push(
        provider
            .attempt(ProviderOperation::LocalSourceListing)
            .no_candidates(ProviderNoCandidateReason::EmptyResult, Some(0)),
    );

    assert!(report.attempted_provider("Local"));
    assert_eq!(
        report.attempts[0].outcome,
        ProviderAttemptOutcome::NoCandidates {
            reason: ProviderNoCandidateReason::EmptyResult,
            candidate_count: Some(0)
        }
    );
}

#[test]
fn provider_run_outcome_pairs_path_and_attempt() {
    let provider = configured_source_providers(&[SourceEntry {
        enabled: true,
        source_type: "folder".into(),
        label: Some("Local library".into()),
        path: Some("/tmp/walls".into()),
        ..SourceEntry::default()
    }])
    .remove(0);

    let outcome = ProviderRunOutcome::applied(
        Some(std::path::PathBuf::from("/tmp/walls/a.jpg")),
        provider
            .attempt(ProviderOperation::AdvanceNext)
            .applied(Some(4)),
    );

    assert_eq!(
        outcome.applied_path.as_deref(),
        Some(std::path::Path::new("/tmp/walls/a.jpg"))
    );
    assert_eq!(outcome.attempt.provider_id, "Local library");
    assert_eq!(
        outcome.attempt.outcome,
        ProviderAttemptOutcome::Applied {
            candidate_count: Some(4)
        }
    );
}
