//! Per-provider config field sets and normalization for `SourceEntry`.
//!
//! Single source of truth for which JSON keys belong to each source type and which
//! fields the TUI should expose. Prevents unrelated `SourceEntry` options from
//! leaking into saved config when types share one serde struct.

use super::{normalize_reddit_source, Secrets, SourceEntry, SourceKind};

/// Hint shown on edit screens for credentials stored outside `config.json`.
pub const SECRETS_EDIT_HINT: &str = "(edit ~/.config/walls/secrets.json)";

/// Which `secrets.json` key a source type depends on, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSecretsKey {
    UnsplashAccessKey,
    RedditClientId,
}

/// Returns the secrets key required for this source type, when credentials live in `secrets.json`.
pub fn source_secrets_key(source_type: &str) -> Option<SourceSecretsKey> {
    match SourceKind::parse(source_type) {
        SourceKind::Unsplash => Some(SourceSecretsKey::UnsplashAccessKey),
        SourceKind::Reddit => Some(SourceSecretsKey::RedditClientId),
        _ => None,
    }
}

pub fn secrets_credential_present(key: SourceSecretsKey, secrets: &Secrets) -> bool {
    match key {
        SourceSecretsKey::UnsplashAccessKey => !secrets.unsplash_access_key.trim().is_empty(),
        SourceSecretsKey::RedditClientId => !secrets.reddit_client_id.trim().is_empty(),
    }
}

pub fn secrets_credential_label(key: SourceSecretsKey) -> &'static str {
    match key {
        SourceSecretsKey::UnsplashAccessKey => "Unsplash access key",
        SourceSecretsKey::RedditClientId => "Reddit API credentials",
    }
}

pub fn secrets_credential_field(key: SourceSecretsKey) -> &'static str {
    match key {
        SourceSecretsKey::UnsplashAccessKey => "unsplash_access_key",
        SourceSecretsKey::RedditClientId => "reddit_client_id",
    }
}

pub fn secrets_credential_warning(key: SourceSecretsKey) -> &'static str {
    match key {
        SourceSecretsKey::UnsplashAccessKey => {
            "warning: Unsplash access key missing in secrets.json (unsplash.com/developers)"
        }
        SourceSecretsKey::RedditClientId => {
            "warning: Reddit API credentials missing in secrets.json (create an app at reddit.com/prefs/apps)"
        }
    }
}

const COMMON_SOURCE_FIELDS: &[&str] = &["enabled", "type", "label"];

/// JSON keys that may be persisted for a given source `type`.
pub fn source_config_fields(source_type: &str) -> &'static [&'static str] {
    match SourceKind::parse(source_type) {
        SourceKind::Reddit => &["enabled", "type", "label", "query", "sort", "time"],
        SourceKind::Folder | SourceKind::Image => &["enabled", "type", "label", "path"],
        SourceKind::Json => &["enabled", "type", "label", "url", "image_path"],
        SourceKind::MediaRss => &["enabled", "type", "label", "url"],
        SourceKind::Attribution => &["enabled", "type", "label", "url", "source", "author"],
        SourceKind::Unsplash => &[
            "enabled",
            "type",
            "label",
            "query",
            "collection",
            "user",
            "topic",
            "orientation",
            "url",
        ],
        SourceKind::Weighting => &["enabled", "type", "label", "query"],
        SourceKind::Wallhaven => &[
            "enabled",
            "type",
            "query",
            "categories",
            "purity",
            "sorting",
            "order",
            "atleast",
            "ratios",
            "broaden_when_cache_below",
            "prefer",
            "collections",
        ],
        SourceKind::Pixabay => &["enabled", "type", "label", "query", "api_key"],
        SourceKind::Immich => &["enabled", "type", "label", "url", "api_key"],
        _ => COMMON_SOURCE_FIELDS,
    }
}

/// Ordered editable fields for the TUI form (subset of config fields).
pub fn source_editable_fields(entry: &SourceEntry) -> Vec<&'static str> {
    let source_kind = SourceKind::parse(&entry.source_type);
    match source_kind {
        SourceKind::Reddit => vec!["enabled", "query", "sort", "time"],
        SourceKind::Folder | SourceKind::Image | SourceKind::Favorites | SourceKind::Fetched => {
            let mut fields = vec!["enabled", "label"];
            if matches!(source_kind, SourceKind::Folder | SourceKind::Image) {
                fields.push("path");
            }
            fields
        }
        SourceKind::Json => vec!["enabled", "label", "url", "image_path"],
        SourceKind::MediaRss => vec!["enabled", "label", "url"],
        SourceKind::Attribution => vec!["enabled", "label", "url", "source", "author"],
        SourceKind::Unsplash => vec![
            "enabled",
            "label",
            "query",
            "collection",
            "user",
            "topic",
            "orientation",
            "url",
        ],
        SourceKind::Weighting => vec!["enabled", "label", "query"],
        SourceKind::Wallhaven => vec![
            "enabled",
            "query",
            "category_general",
            "category_anime",
            "category_people",
            "purity_sfw",
            "purity_sketchy",
            "purity_nsfw",
            "sorting",
            "order",
            "ratios",
            "atleast",
            "broaden_when_cache_below",
            "prefer",
            "collections",
        ],
        SourceKind::Pixabay => vec!["enabled", "label", "query", "api_key"],
        SourceKind::Immich => vec!["enabled", "label", "url", "api_key"],
        _ => vec!["enabled", "label"],
    }
}

/// Whether an allowed source field should preserve an explicitly empty string.
///
/// Most source fields use empty input to clear an optional JSON key. Wallhaven's
/// query is different: an empty query means "search Wallhaven using filters
/// only", so it must survive edits and normalization as `""`.
pub fn source_field_preserves_blank(source_type: &str, key: &str) -> bool {
    matches!(
        (SourceKind::parse(source_type), key),
        (SourceKind::Wallhaven, "query")
    )
}

/// Strip fields that do not belong to this source type and apply type-specific cleanup.
pub fn normalize_source_entry(entry: &mut SourceEntry) {
    if SourceKind::parse(&entry.source_type) == SourceKind::Reddit {
        normalize_reddit_source(entry);
    }
    let is_wallhaven = SourceKind::parse(&entry.source_type) == SourceKind::Wallhaven;

    let allowed: std::collections::HashSet<&str> = source_config_fields(entry.source_type.as_str())
        .iter()
        .copied()
        .collect();

    normalize_optional_field(&allowed, "label", &mut entry.label);
    normalize_optional_field(&allowed, "path", &mut entry.path);
    if source_field_preserves_blank(&entry.source_type, "query") && allowed.contains("query") {
        normalize_blank_preserving_field(&mut entry.query);
    } else {
        normalize_optional_field(&allowed, "query", &mut entry.query);
    }
    normalize_optional_field(&allowed, "url", &mut entry.url);
    normalize_optional_field(&allowed, "collection", &mut entry.collection);
    normalize_optional_field(&allowed, "user", &mut entry.user);
    normalize_optional_field(&allowed, "topic", &mut entry.topic);
    normalize_optional_field(&allowed, "orientation", &mut entry.orientation);
    normalize_optional_field(&allowed, "api_key", &mut entry.api_key);
    normalize_optional_field(&allowed, "image_path", &mut entry.image_path);
    normalize_optional_field(&allowed, "source", &mut entry.source);
    normalize_optional_field(&allowed, "author", &mut entry.author);
    normalize_optional_field(&allowed, "sort", &mut entry.sort);
    normalize_optional_field(&allowed, "time", &mut entry.time);
    normalize_optional_field(&allowed, "categories", &mut entry.categories);
    normalize_optional_field(&allowed, "purity", &mut entry.purity);
    normalize_optional_field(&allowed, "sorting", &mut entry.sorting);
    normalize_optional_field(&allowed, "order", &mut entry.order);
    normalize_optional_field(&allowed, "atleast", &mut entry.atleast);
    normalize_optional_field(&allowed, "ratios", &mut entry.ratios);
    if !allowed.contains("prefer") {
        entry.prefer = None;
    }
    if !allowed.contains("broaden_when_cache_below") {
        entry.broaden_when_cache_below = None;
    }
    if !allowed.contains("collections") {
        entry.collections.clear();
    }
    if is_wallhaven {
        super::wallhaven::populate_wallhaven_source_defaults(entry);
    }

    // serde compat only; never persisted from walls edits.
    entry.title_path = None;
}

fn normalize_optional_field(
    allowed: &std::collections::HashSet<&str>,
    key: &str,
    value: &mut Option<String>,
) {
    if allowed.contains(key) {
        *value = clean_optional(value.take());
    } else {
        *value = None;
    }
}

fn normalize_blank_preserving_field(value: &mut Option<String>) {
    *value = value.take().map(|value| value.trim().to_string());
}

pub fn normalize_config_sources(sources: &mut [SourceEntry]) {
    for entry in sources {
        normalize_source_entry(entry);
    }
}

/// Detail lines for the sources subnav when a secrets-backed provider is selected.
pub fn source_secrets_detail_lines(
    entry: &SourceEntry,
    secrets: &Secrets,
    internet_enabled: bool,
) -> Vec<String> {
    if SourceKind::parse(&entry.source_type) == SourceKind::Wallhaven {
        return vec![format!(
            "wallhaven api key: {}",
            if secrets.wallhaven_api_key.trim().is_empty() {
                "missing"
            } else {
                "present"
            }
        )];
    }

    let Some(key) = source_secrets_key(&entry.source_type) else {
        return Vec::new();
    };
    let present = secrets_credential_present(key, secrets);
    let mut lines = vec![format!(
        "{}: {}",
        secrets_credential_label(key).to_ascii_lowercase(),
        if present { "present" } else { "missing" }
    )];
    if entry.enabled && internet_enabled && !present {
        lines.push(secrets_credential_warning(key).to_string());
    }
    lines
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_reddit_strips_unrelated_fields() {
        let mut entry = SourceEntry {
            enabled: true,
            source_type: "reddit".into(),
            label: Some("Old label".into()),
            path: Some("/tmp".into()),
            query: Some("wallpapers".into()),
            url: Some("https://example.com".into()),
            api_key: Some("secret".into()),
            sort: Some("hot".into()),
            time: None,
            collection: None,
            user: None,
            topic: None,
            orientation: None,
            image_path: None,
            title_path: None,
            ..SourceEntry::default()
        };
        normalize_source_entry(&mut entry);
        assert_eq!(entry.query.as_deref(), Some("wallpapers"));
        assert_eq!(entry.sort.as_deref(), Some("hot"));
        assert!(entry.path.is_none());
        assert!(entry.url.is_none());
        assert!(entry.api_key.is_none());
        assert_eq!(entry.label.as_deref(), Some("Old label"));
    }

    #[test]
    fn normalize_unsplash_keeps_only_unsplash_fields() {
        let mut entry = SourceEntry {
            enabled: false,
            source_type: "unsplash".into(),
            label: Some("Nature".into()),
            query: Some("forest".into()),
            orientation: Some("landscape".into()),
            path: Some("/nope".into()),
            sort: Some("hot".into()),
            url: None,
            collection: None,
            user: None,
            topic: None,
            api_key: None,
            image_path: None,
            title_path: None,
            time: None,
            ..SourceEntry::default()
        };
        normalize_source_entry(&mut entry);
        assert_eq!(entry.query.as_deref(), Some("forest"));
        assert_eq!(entry.orientation.as_deref(), Some("landscape"));
        assert!(entry.path.is_none());
        assert!(entry.sort.is_none());
    }

    #[test]
    fn normalize_wallhaven_keeps_query_and_strips_label() {
        let mut entry = SourceEntry {
            enabled: true,
            source_type: "wallhaven".into(),
            label: Some("Old label".into()),
            query: Some("jupiter".into()),
            path: Some("/nope".into()),
            url: Some("https://example.com".into()),
            api_key: Some("secret".into()),
            sort: Some("hot".into()),
            time: None,
            collection: None,
            user: None,
            topic: None,
            orientation: None,
            image_path: None,
            title_path: None,
            source: None,
            author: None,
            categories: Some("000".into()),
            purity: Some("000".into()),
            sorting: Some("date".into()),
            order: Some("asc".into()),
            ratios: Some("16x10".into()),
            atleast: Some("1024x768".into()),
            broaden_when_cache_below: Some(2),
            prefer: Some(crate::config::WallhavenPrefer::SearchOnly),
            collections: Vec::new(),
        };
        normalize_source_entry(&mut entry);
        assert_eq!(entry.query.as_deref(), Some("jupiter"));
        assert_eq!(entry.categories.as_deref(), Some("000"));
        assert_eq!(entry.purity.as_deref(), Some("000"));
        assert_eq!(entry.sorting.as_deref(), Some("date"));
        assert_eq!(entry.order.as_deref(), Some("asc"));
        assert_eq!(entry.ratios.as_deref(), Some("16x10"));
        assert_eq!(entry.atleast.as_deref(), Some("1024x768"));
        assert_eq!(entry.broaden_when_cache_below, Some(2));
        assert_eq!(
            entry.prefer,
            Some(crate::config::WallhavenPrefer::SearchOnly)
        );
        assert!(entry.label.is_none());
        assert!(entry.path.is_none());
        assert!(entry.url.is_none());
        assert!(entry.api_key.is_none());
        assert!(entry.sort.is_none());
    }

    #[test]
    fn normalize_wallhaven_preserves_empty_query() {
        let mut entry = SourceEntry {
            enabled: true,
            source_type: "wallhaven".into(),
            query: Some("   ".into()),
            ..SourceEntry::default()
        };

        normalize_source_entry(&mut entry);

        assert_eq!(entry.query.as_deref(), Some(""));
    }

    #[test]
    fn normalize_attribution_keeps_source_and_author_metadata() {
        let mut entry = SourceEntry {
            enabled: true,
            source_type: "attribution".into(),
            label: Some("Daily image".into()),
            url: Some("https://example.com/wall.jpg".into()),
            source: Some("NASA Image Library".into()),
            author: Some("Hubble".into()),
            query: Some("should drop".into()),
            path: Some("/nope".into()),
            title_path: Some("$.title".into()),
            ..SourceEntry::default()
        };
        normalize_source_entry(&mut entry);
        assert_eq!(entry.label.as_deref(), Some("Daily image"));
        assert_eq!(entry.url.as_deref(), Some("https://example.com/wall.jpg"));
        assert_eq!(entry.source.as_deref(), Some("NASA Image Library"));
        assert_eq!(entry.author.as_deref(), Some("Hubble"));
        assert!(entry.query.is_none());
        assert!(entry.path.is_none());
        assert!(entry.title_path.is_none());

        entry.source_type = "json".into();
        normalize_source_entry(&mut entry);
        assert!(entry.source.is_none());
        assert!(entry.author.is_none());
    }

    #[test]
    fn source_secrets_key_maps_internet_providers() {
        assert_eq!(
            source_secrets_key("reddit"),
            Some(SourceSecretsKey::RedditClientId)
        );
        assert_eq!(
            source_secrets_key("unsplash"),
            Some(SourceSecretsKey::UnsplashAccessKey)
        );
        assert_eq!(source_secrets_key("wallhaven"), None);
        assert_eq!(source_secrets_key("bing"), None);
    }

    #[test]
    fn wallhaven_source_details_show_optional_api_key_presence() {
        let source = SourceEntry {
            enabled: true,
            source_type: "wallhaven".into(),
            query: Some("space".into()),
            ..SourceEntry::default()
        };
        let mut secrets = Secrets::default();

        assert_eq!(
            source_secrets_detail_lines(&source, &secrets, true),
            ["wallhaven api key: missing"]
        );

        secrets.wallhaven_api_key = "key".into();
        assert_eq!(
            source_secrets_detail_lines(&source, &secrets, true),
            ["wallhaven api key: present"]
        );
    }
}
