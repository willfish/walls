mod common {
    include!("common/minimal.rs");
}

use walls_core::validate::{validate_config, validate_source_edit, validate_wallhaven_edit};
use walls_core::WallsCtx;

fn load_config_json(root: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root.join("config.json")).unwrap()).unwrap()
}

fn validate_root(root: &std::path::Path) -> Vec<String> {
    let ctx = WallsCtx::load_from(root).unwrap();
    validate_config(&ctx.config, &ctx.secrets, &ctx.paths)
}

#[test]
fn validate_config_ok_for_minimal_config() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let errors = validate_root(root.path());
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn validate_config_reports_missing_folder_path() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["sources"] = serde_json::json!([{
        "enabled": true,
        "type": "folder",
        "path": "/nonexistent/walls-test-folder"
    }]);
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(errors.iter().any(|e| e.contains("does not exist")));
}

#[test]
fn validate_config_reports_missing_reddit_credentials() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["change"]["internet_enabled"] = serde_json::json!(true);
    config["sources"] = serde_json::json!([{
        "enabled": true,
        "type": "reddit",
        "query": "wallpapers",
        "sort": "hot"
    }]);
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("secrets.reddit_client_id is empty")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_missing_unsplash_key() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["change"]["internet_enabled"] = serde_json::json!(true);
    config["sources"] = serde_json::json!([{
        "enabled": true,
        "type": "unsplash",
        "query": "forest"
    }]);
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("secrets.unsplash_access_key is empty")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_enabled_provider_schema_errors() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["change"]["internet_enabled"] = serde_json::json!(true);
    config["sources"] = serde_json::json!([
        { "enabled": true, "type": "future-provider", "label": "Future" },
        { "enabled": true, "type": "reddit", "label": "Reddit", "sort": "sideways", "time": "decade" },
        { "enabled": true, "type": "unsplash", "label": "Unsplash", "orientation": "diagonal" },
        { "enabled": true, "type": "json", "label": "JSON" },
        { "enabled": true, "type": "json", "label": "Bad JSON", "url": "ftp://example.com/feed.json", "image_path": "download_url" },
        { "enabled": true, "type": "mediarss", "label": "RSS", "url": "not a url" },
        { "enabled": true, "type": "attribution", "label": "Attribution" },
        { "enabled": true, "type": "pixabay", "label": "Pixabay" },
        { "enabled": true, "type": "immich", "label": "Immich", "url": "https://immich.example" },
        { "enabled": true, "type": "spotlight", "label": "Spotlight" },
        { "enabled": true, "type": "weighting", "label": "Weighting" }
    ]);
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("unsupported source type \"future-provider\"")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"Reddit\"): query is required")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"Reddit\").sort must be one of")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"Reddit\").time must be one of")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"Unsplash\").orientation must be one of")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"JSON\"): url is required")),
        "{errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.contains("source Some(\"Bad JSON\").url must use http or https")
        }),
        "{errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.contains("source Some(\"Bad JSON\").image_path must be a JSON path")
        }),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"RSS\").url must be a valid URL")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"Attribution\"): url is required")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"Pixabay\"): api_key is required")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"Immich\"): api_key is required")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("spotlight source requires path or url")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"Weighting\"): query is required")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_accepts_valid_provider_source_schemas() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let spotlight = root.path().join("spotlight");
    std::fs::create_dir_all(&spotlight).unwrap();
    let mut config = load_config_json(root.path());
    config["change"]["internet_enabled"] = serde_json::json!(true);
    config["sources"] = serde_json::json!([
        { "enabled": true, "type": "reddit", "label": "Reddit", "query": "wallpapers", "sort": "top", "time": "month" },
        { "enabled": true, "type": "unsplash", "label": "Unsplash", "orientation": "landscape", "query": "forest" },
        { "enabled": true, "type": "bing", "label": "Bing" },
        { "enabled": true, "type": "apod", "label": "APOD" },
        { "enabled": true, "type": "json", "label": "JSON", "url": "https://example.com/feed.json", "image_path": "$.download_url" },
        { "enabled": true, "type": "mediarss", "label": "RSS", "url": "https://example.com/feed.xml" },
        { "enabled": true, "type": "attribution", "label": "Attribution", "url": "https://example.com/wall.jpg" },
        { "enabled": true, "type": "pixabay", "label": "Pixabay", "api_key": "pixabay-key" },
        { "enabled": true, "type": "immich", "label": "Immich", "url": "https://immich.example", "api_key": "immich-key" },
        { "enabled": true, "type": "spotlight", "label": "Spotlight", "path": spotlight.display().to_string() },
        { "enabled": true, "type": "weighting", "label": "Weighting", "query": "high" }
    ]);
    common::write_config(root.path(), config);
    common::write_secrets(
        root.path(),
        serde_json::json!({
            "reddit_client_id": "reddit-key",
            "unsplash_access_key": "unsplash-key"
        }),
    );

    let errors = validate_root(root.path());
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn validate_config_skips_disabled_provider_schema_errors() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["change"]["internet_enabled"] = serde_json::json!(true);
    config["sources"] = serde_json::json!([
        { "enabled": false, "type": "future-provider" },
        { "enabled": false, "type": "json" },
        { "enabled": false, "type": "immich", "url": "not a url" },
        { "enabled": false, "type": "" }
    ]);
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn source_edit_validate_reports_only_selected_provider_schema_errors() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["sources"] = serde_json::json!([
        { "enabled": true, "type": "json", "label": "Selected" },
        { "enabled": true, "type": "immich", "label": "Other" }
    ]);
    common::write_config(root.path(), config);

    let ctx = WallsCtx::load_from(root.path()).unwrap();
    let errors = validate_source_edit(0, &ctx.config, &ctx.secrets, &ctx.paths);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("source Some(\"Selected\"): url is required")),
        "{errors:?}"
    );
    assert!(
        !errors.iter().any(|error| error.contains("Other")),
        "scoped source validation should not report unrelated provider errors: {errors:?}"
    );
}

#[test]
fn validate_config_reports_missing_custom_script_for_custom_script_backend() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let missing = root.path().join("missing-script.sh");
    let mut config = load_config_json(root.path());
    config["apply"]["custom_script"] = serde_json::json!(missing.display().to_string());
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("apply.custom_script not found or not a file")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_required_custom_script_for_custom_script_backend() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["apply"]["custom_script"] = serde_json::Value::Null;
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors.iter().any(|error| error
            .contains("apply.custom_script is required when apply.backend is custom-script")),
        "{errors:?}"
    );
}

#[cfg(unix)]
#[test]
fn validate_config_reports_non_executable_custom_script_on_unix() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    std::fs::set_permissions(&noop, std::fs::Permissions::from_mode(0o644)).unwrap();
    common::write_minimal_config(root.path(), &images, &noop);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("apply.custom_script is not executable")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_custom_script_when_backend_does_not_use_it() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["apply"]["backend"] = serde_json::json!("gnome");
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("apply.custom_script is set but apply.backend is gnome")),
        "{errors:?}"
    );
}

#[test]
fn source_edit_validate_ignores_other_enabled_sources_with_missing_paths() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["sources"] = serde_json::json!([
        { "enabled": true, "type": "favorites", "label": "Favorites" },
        {
            "enabled": true,
            "type": "folder",
            "label": "Missing",
            "path": "/nonexistent/walls-test-folder"
        }
    ]);
    common::write_config(root.path(), config);

    let ctx = WallsCtx::load_from(root.path()).unwrap();
    let full = validate_config(&ctx.config, &ctx.secrets, &ctx.paths);
    assert!(
        full.iter().any(|e| e.contains("does not exist")),
        "{full:?}"
    );

    let scoped = validate_source_edit(0, &ctx.config, &ctx.secrets, &ctx.paths);
    assert!(
        scoped.is_empty(),
        "favorites should not be blocked by another source's missing folder path: {scoped:?}"
    );
}

#[test]
fn source_edit_validate_reports_empty_type() {
    let root = tempfile::tempdir().unwrap();
    let config = serde_json::from_value::<walls_core::config::Config>(serde_json::json!({
        "paths": {
            "cache_dir": root.path().join("cache").display().to_string(),
            "download_dir": root.path().join("downloaded").display().to_string(),
            "favorites_dir": root.path().join("favorites").display().to_string(),
            "fetched_dir": root.path().join("fetched").display().to_string(),
            "compose_dir": root.path().join("wallpaper").display().to_string()
        },
        "sources": [
            { "enabled": true, "type": "", "label": "Broken" }
        ]
    }))
    .unwrap();
    let paths = walls_core::paths::WallsPaths {
        config_dir: root.path().to_path_buf(),
        config_file: root.path().join("config.json"),
        secrets_file: root.path().join("secrets.json"),
        state_file: root.path().join("state.json"),
        cache_dir: root.path().join("cache"),
        download_dir: root.path().join("downloaded"),
        favorites_dir: root.path().join("favorites"),
        fetched_dir: root.path().join("fetched"),
        compose_dir: root.path().join("wallpaper"),
    };

    let errors = validate_source_edit(0, &config, &walls_core::config::Secrets::default(), &paths);
    assert!(
        errors.iter().any(|e| e.contains("type is required")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_missing_favorites_dir_when_enabled() {
    let root = tempfile::tempdir().unwrap();
    let missing_favorites = root.path().join("missing-favorites");
    let config = serde_json::from_value::<walls_core::config::Config>(serde_json::json!({
        "paths": {
            "cache_dir": root.path().join("cache").display().to_string(),
            "download_dir": root.path().join("downloaded").display().to_string(),
            "favorites_dir": missing_favorites.display().to_string(),
            "fetched_dir": root.path().join("fetched").display().to_string(),
            "compose_dir": root.path().join("wallpaper").display().to_string()
        },
        "sources": [
            { "enabled": true, "type": "favorites", "label": "Favorites" }
        ]
    }))
    .unwrap();
    let paths = walls_core::paths::WallsPaths {
        config_dir: root.path().to_path_buf(),
        config_file: root.path().join("config.json"),
        secrets_file: root.path().join("secrets.json"),
        state_file: root.path().join("state.json"),
        cache_dir: root.path().join("cache"),
        download_dir: root.path().join("downloaded"),
        favorites_dir: missing_favorites,
        fetched_dir: root.path().join("fetched"),
        compose_dir: root.path().join("wallpaper"),
    };

    let errors = validate_config(&config, &walls_core::config::Secrets::default(), &paths);
    assert!(
        errors.iter().any(|e| e.contains("does not exist")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_skips_disabled_folder_with_missing_path() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["sources"] = serde_json::json!([{
        "enabled": false,
        "type": "folder",
        "label": "System backgrounds",
        "path": "/usr/share/backgrounds"
    }]);
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        !errors.iter().any(|e| e.contains("does not exist")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_zero_quota_size() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["quota"] = serde_json::json!({ "enabled": true, "size_mb": 0 });
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("quota.size_mb must be greater than zero")),
        "{errors:?}"
    );
}

#[test]
fn validate_config_reports_invalid_wallhaven_provider_settings() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["wallhaven"] = serde_json::json!({
        "enabled": true,
        "collections": [
            { "username": "", "id": 0 }
        ],
        "search": {
            "q": "forest",
            "categories": "12",
            "purity": "000",
            "sorting": "popular",
            "order": "sideways",
            "atleast": "large"
        }
    });
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(
        errors
            .iter()
            .any(|error| error.contains("wallhaven.search.categories must be three binary digits")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("wallhaven.search.purity must enable at least one option")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("wallhaven.search.sorting must be one of")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("wallhaven.search.order must be one of")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("wallhaven.search.atleast must use WIDTHxHEIGHT format")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("wallhaven.collections[0].username must not be empty")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("wallhaven.collections[0].id must be greater than zero")),
        "{errors:?}"
    );
}

#[test]
fn validate_wallhaven_edit_reports_provider_errors_without_global_config_checks() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["sources"] = serde_json::json!([{
        "enabled": true,
        "type": "folder",
        "label": "Missing",
        "path": "/nonexistent/walls-test-folder"
    }]);
    config["wallhaven"] = serde_json::json!({
        "enabled": true,
        "search": {
            "categories": "abc",
            "purity": "001",
            "sorting": "random",
            "order": "desc",
            "atleast": "1920x1080"
        }
    });
    common::write_config(root.path(), config);

    let ctx = WallsCtx::load_from(root.path()).unwrap();
    let errors = validate_wallhaven_edit(&ctx.config, &ctx.secrets);

    assert!(
        errors
            .iter()
            .any(|error| error.contains("wallhaven.search.categories must be three binary digits")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("cannot select only NSFW without")),
        "{errors:?}"
    );
    assert!(
        !errors.iter().any(|error| error.contains("does not exist")),
        "scoped Wallhaven edit validation should not report unrelated source errors: {errors:?}"
    );
}

#[test]
fn validate_config_allows_keyless_wallhaven_with_safe_purity() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["wallhaven"] = serde_json::json!({
        "enabled": true,
        "search": {
            "q": "forest",
            "categories": "111",
            "purity": "100",
            "sorting": "random",
            "order": "desc",
            "atleast": "1920x1080"
        }
    });
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn validate_config_skips_disabled_wallhaven_provider_settings() {
    let root = tempfile::tempdir().unwrap();
    let images = root.path().join("images");
    std::fs::create_dir_all(&images).unwrap();
    let noop = common::write_noop_script(root.path());
    common::write_minimal_config(root.path(), &images, &noop);

    let mut config = load_config_json(root.path());
    config["wallhaven"] = serde_json::json!({
        "enabled": false,
        "collections": [
            { "username": "", "id": 0 }
        ],
        "search": {
            "categories": "abc",
            "purity": "000",
            "sorting": "popular",
            "order": "sideways",
            "atleast": "large"
        }
    });
    common::write_config(root.path(), config);

    let errors = validate_root(root.path());
    assert!(errors.is_empty(), "{errors:?}");
}
