use std::fs;

use walls_core::apply::ApplyTrigger;
use walls_core::events::{read_events, EventKind};
use walls_core::providers::{
    ProviderAttemptOutcome, ProviderKind, ProviderNoCandidateReason, ProviderStatus,
};
use walls_core::WallsCtx;

fn write_test_config(
    root: &std::path::Path,
    image_dir: &std::path::Path,
    noop_script: &std::path::Path,
) {
    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": false },
        "paths": {
            "cache_dir": root.join("cache").display().to_string(),
            "download_dir": root.join("downloaded").display().to_string(),
            "favorites_dir": root.join("favorites").display().to_string(),
            "fetched_dir": root.join("fetched").display().to_string(),
            "compose_dir": root.join("wallpaper").display().to_string(),
        },
        "apply": {
            "backend": "custom-script",
            "custom_script": noop_script.display().to_string(),
        },
        "display": { "mode": "os" },
        "selection": { "avoid_recent": 50 },
        "sources": [
            { "enabled": true, "type": "folder", "path": image_dir.display().to_string() }
        ],
    });
    fs::write(
        root.join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(root.join("secrets.json"), "{}").unwrap();
}

#[tokio::test]
async fn advance_next_writes_state() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("wall.jpg"), b"fake jpeg").unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_test_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx
        .advance_next()
        .await
        .unwrap()
        .expect("should apply one image");
    assert!(applied.ends_with("wall.jpg"));
    assert!(ctx.state.current.is_some());
    assert_eq!(ctx.state.history.len(), 1);
    assert_eq!(ctx.state.history[0], applied.display().to_string());
    let events = read_events(&ctx.paths.event_journal_file).unwrap();
    assert!(events.iter().any(|event| matches!(
        event.kind,
        EventKind::Apply {
            trigger: ApplyTrigger::Auto,
            ..
        }
    )));
    assert!(events
        .iter()
        .any(|event| matches!(event.kind, EventKind::ProviderAttempt { .. })));
}

#[tokio::test]
async fn advance_next_manual_runs_when_paused() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("wall.jpg"), b"fake jpeg").unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_test_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.set_paused(true).unwrap();
    let applied = ctx
        .advance_next_manual()
        .await
        .unwrap()
        .expect("manual next should apply even when paused");
    assert!(applied.ends_with("wall.jpg"));
}

#[tokio::test]
async fn advance_next_skips_when_paused() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("wall.jpg"), b"fake jpeg").unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_test_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    ctx.set_paused(true).unwrap();
    let applied = ctx.advance_next().await.unwrap();
    assert!(applied.is_none());
    assert!(ctx
        .provider_status_report
        .attempts
        .iter()
        .any(|attempt| matches!(
            attempt.outcome,
            ProviderAttemptOutcome::Skipped {
                reason: ProviderNoCandidateReason::Disabled
            }
        )));
}

#[tokio::test]
async fn advance_next_returns_none_when_local_sources_are_empty() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_test_config(root.path(), &images, &noop);

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx.advance_next().await.unwrap();

    assert!(applied.is_none());
    assert!(ctx.state.current.is_none());
    assert!(ctx.state.history.is_empty());
    assert!(ctx.provider_status_report.attempts.iter().any(|attempt| {
        attempt.provider_id == "local"
            && matches!(
                attempt.outcome,
                ProviderAttemptOutcome::NoCandidates {
                    reason: ProviderNoCandidateReason::EmptyResult,
                    candidate_count: Some(0)
                }
            )
    }));
}

#[tokio::test]
async fn advance_next_reports_missing_unsplash_credentials() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": true },
        "paths": {
            "cache_dir": root.path().join("cache").display().to_string(),
            "download_dir": root.path().join("downloaded").display().to_string(),
            "favorites_dir": root.path().join("favorites").display().to_string(),
            "fetched_dir": root.path().join("fetched").display().to_string(),
            "compose_dir": root.path().join("wallpaper").display().to_string(),
        },
        "apply": {
            "backend": "custom-script",
            "custom_script": noop.display().to_string(),
        },
        "display": { "mode": "os" },
        "wallhaven": { "enabled": false },
        "sources": [
            { "enabled": true, "type": "unsplash", "query": "forest" }
        ],
    });
    fs::write(
        root.path().join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(root.path().join("secrets.json"), "{}").unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx.advance_next().await.unwrap();

    assert!(applied.is_none());
    let unsplash = ctx
        .provider_status_report
        .attempts
        .iter()
        .find(|attempt| attempt.provider_kind == ProviderKind::Unsplash)
        .expect("unsplash attempt");
    assert_eq!(unsplash.status, ProviderStatus::CredentialMissing);
    assert_eq!(unsplash.fallback_provider_id.as_deref(), Some("wallhaven"));
    assert_eq!(
        unsplash.outcome,
        ProviderAttemptOutcome::Skipped {
            reason: ProviderNoCandidateReason::CredentialMissing
        }
    );
}

#[tokio::test]
async fn advance_next_reports_inline_provider_offline_skip() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": false },
        "paths": {
            "cache_dir": root.path().join("cache").display().to_string(),
            "download_dir": root.path().join("downloaded").display().to_string(),
            "favorites_dir": root.path().join("favorites").display().to_string(),
            "fetched_dir": root.path().join("fetched").display().to_string(),
            "compose_dir": root.path().join("wallpaper").display().to_string(),
        },
        "apply": {
            "backend": "custom-script",
            "custom_script": noop.display().to_string(),
        },
        "display": { "mode": "os" },
        "wallhaven": { "enabled": false },
        "sources": [
            { "enabled": true, "type": "reddit", "subreddit": "EarthPorn" },
            { "enabled": true, "type": "folder", "path": images.display().to_string() }
        ],
    });
    fs::write(
        root.path().join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(root.path().join("secrets.json"), "{}").unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx.advance_next().await.unwrap();

    assert!(applied.is_none());
    let reddit = ctx
        .provider_status_report
        .attempts
        .iter()
        .find(|attempt| attempt.provider_kind == ProviderKind::Reddit)
        .expect("reddit attempt");
    assert_eq!(reddit.status, ProviderStatus::OfflineDisabled);
    assert_eq!(reddit.fallback_provider_id.as_deref(), Some("apod"));
    assert_eq!(
        reddit.outcome,
        ProviderAttemptOutcome::Skipped {
            reason: ProviderNoCandidateReason::OfflineDisabled
        }
    );
}

#[tokio::test]
async fn advance_next_avoids_recent_candidates_across_many_local_files() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    fs::write(images.join("recent-a.jpg"), b"fake jpeg").unwrap();
    fs::write(images.join("recent-b.jpg"), b"fake jpeg").unwrap();
    fs::write(images.join("available.jpg"), b"fake jpeg").unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }
    write_test_config(root.path(), &images, &noop);
    fs::write(
        root.path().join("state.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "history": [
                images.join("recent-a.jpg").display().to_string(),
                images.join("recent-b.jpg").display().to_string()
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx
        .advance_next()
        .await
        .unwrap()
        .expect("should apply the only non-recent image");

    assert!(applied.ends_with("available.jpg"));
    assert_eq!(ctx.state.history[0], applied.display().to_string());
}

// TDD RED test for wiring a real provider (Bing - public no-key).
// We expect advance_next with only a "bing" source + internet_enabled to fetch a real
// image from Bing's public endpoint and apply it. Currently will be None (no path for bing).
//
// Ignored by default: live network fetch against public endpoint. These fail under
// --offline (as used in nix checkPhase for hermetic derivations) and can be flaky in CI.
// Run with `cargo test -- --ignored` for manual end-to-end provider proof.
#[ignore = "live network; skipped in offline/hermetic CI nix checks"]
#[tokio::test]
async fn advance_next_with_bing_source_fetches_real_bing_wallpaper() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": true },
        "paths": {
            "cache_dir": root.path().join("cache").display().to_string(),
            "download_dir": root.path().join("downloaded").display().to_string(),
            "favorites_dir": root.path().join("favorites").display().to_string(),
            "fetched_dir": root.path().join("fetched").display().to_string(),
            "compose_dir": root.path().join("wallpaper").display().to_string(),
        },
        "apply": {
            "backend": "custom-script",
            "custom_script": noop.display().to_string(),
        },
        "display": { "mode": "os" },
        "selection": { "avoid_recent": 50 },
        "sources": [
            { "enabled": true, "type": "bing", "label": "Bing daily" }
        ]
    });
    fs::write(
        root.path().join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(root.path().join("secrets.json"), "{}").unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();

    // Feature the providers API: even before advance, the bing source is classified correctly.
    let descs = walls_core::providers::configured_source_providers(&ctx.config.sources);
    assert_eq!(descs.len(), 1);
    assert_eq!(descs[0].kind, walls_core::providers::ProviderKind::Bing);
    assert!(descs[0]
        .capabilities
        .contains(&walls_core::providers::ProviderCapability::Download));

    let applied = ctx.advance_next().await.unwrap();

    // Proves the end-to-end for a real provider: live Bing fetch + apply works when
    // configured with type: "bing" (uses the public classification + the minimal wiring).
    assert!(
        applied.is_some(),
        "bing source should have delivered a real wallpaper"
    );
    let p = applied.unwrap();
    assert!(p.exists(), "applied path should exist on disk");
    assert!(
        p.to_string_lossy().ends_with("bing-daily.jpg"),
        "our bing impl uses this cache name; got {}",
        p.display()
    );
}

// TDD RED tests for additional source examples (JSON feed, MediaRSS).
// These will fail until we add apply_json_feed / apply_media_rss in advance (similar to bing).
// The source examples point to public feeds that these will support.
//
// Ignored by default: live network fetch against public endpoint. These fail under
// --offline (as used in nix checkPhase for hermetic derivations) and can be flaky in CI.
// Run with `cargo test -- --ignored` for manual end-to-end provider proof.
#[ignore = "live network; skipped in offline/hermetic CI nix checks"]
#[tokio::test]
async fn advance_next_with_json_source_fetches_real_image_from_feed() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    // Use the same public json as the source example (picsum info has $.download_url)
    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": true },
        "paths": {
            "cache_dir": root.path().join("cache").display().to_string(),
            "download_dir": root.path().join("downloaded").display().to_string(),
            "favorites_dir": root.path().join("favorites").display().to_string(),
            "fetched_dir": root.path().join("fetched").display().to_string(),
            "compose_dir": root.path().join("wallpaper").display().to_string(),
        },
        "apply": {
            "backend": "custom-script",
            "custom_script": noop.display().to_string(),
        },
        "display": { "mode": "os" },
        "selection": { "avoid_recent": 50 },
        "sources": [
            { "enabled": true, "type": "json", "url": "https://picsum.photos/id/1015/info", "image_path": "$.download_url", "label": "Demo JSON" }
        ]
    });
    fs::write(
        root.path().join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(root.path().join("secrets.json"), "{}").unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx.advance_next().await.unwrap();

    assert!(
        applied.is_some(),
        "json source should deliver real image from feed"
    );
    let p = applied.unwrap();
    assert!(p.exists());
}

// Ignored by default: live network fetch against public endpoint. These fail under
// --offline (as used in nix checkPhase for hermetic derivations) and can be flaky in CI.
// Run with `cargo test -- --ignored` for manual end-to-end provider proof.
#[ignore = "live network; skipped in offline/hermetic CI nix checks"]
#[tokio::test]
async fn advance_next_with_mediarss_source_fetches_real_image_from_rss() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    fs::create_dir_all(&images).unwrap();
    let noop = root.path().join("noop.sh");
    fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let config = serde_json::json!({
        "change": { "enabled": true, "internet_enabled": true },
        "paths": {
            "cache_dir": root.path().join("cache").display().to_string(),
            "download_dir": root.path().join("downloaded").display().to_string(),
            "favorites_dir": root.path().join("favorites").display().to_string(),
            "fetched_dir": root.path().join("fetched").display().to_string(),
            "compose_dir": root.path().join("wallpaper").display().to_string(),
        },
        "apply": {
            "backend": "custom-script",
            "custom_script": noop.display().to_string(),
        },
        "display": { "mode": "os" },
        "selection": { "avoid_recent": 50 },
        "sources": [
            { "enabled": true, "type": "mediarss", "url": "https://www.nasa.gov/rss/dyn/lg_image_of_the_day.rss", "label": "NASA RSS" }
        ]
    });
    fs::write(
        root.path().join("config.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    fs::write(root.path().join("secrets.json"), "{}").unwrap();

    let mut ctx = WallsCtx::load_from(root.path()).unwrap();
    let applied = ctx.advance_next().await.unwrap();

    assert!(applied.is_some(), "mediarss should deliver from RSS feed");
    let p = applied.unwrap();
    assert!(p.exists());
}
