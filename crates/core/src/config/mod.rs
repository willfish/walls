use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

mod apply;
mod display;
mod reddit;
mod source_kind;
mod source_schema;
mod unsplash;
mod wallhaven;

pub use apply::{
    ApplyBackendSetting, ApplyConfig, CosmicApplyConfig, CosmicBackgroundEntryConfig, CosmicMethod,
};
pub use display::{DisplayConfig, DisplayFiltersConfig, ImageMagickFilterConfig};
pub use reddit::{
    normalize_reddit_source, reddit_json_url, reddit_listing_url, reddit_oauth_listing_url,
    reddit_sort_needs_time, reddit_sort_value, reddit_subreddit, reddit_summary, reddit_time_value,
    REDDIT_SORT_CHOICES, REDDIT_TIME_CHOICES,
};
pub use source_kind::SourceKind;
pub use source_schema::{
    normalize_config_sources, normalize_source_entry, secrets_credential_field,
    secrets_credential_label, secrets_credential_present, secrets_credential_warning,
    source_config_fields, source_editable_fields, source_secrets_detail_lines, source_secrets_key,
    SourceSecretsKey, SECRETS_EDIT_HINT,
};
pub use unsplash::UnsplashSourceConfig;
pub use wallhaven::{
    wallhaven_resolution_choices, wallhaven_resolution_supported, WallhavenCollection,
    WallhavenConfig, WallhavenPrefer, WallhavenSearch, WALLHAVEN_DEFAULT_QUERY,
    WALLHAVEN_FALLBACK_RESOLUTION,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub change: ChangeConfig,
    pub paths: PathsConfig,
    #[serde(default)]
    pub quota: QuotaConfig,
    #[serde(default)]
    pub apply: ApplyConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub selection: SelectionConfig,
    #[serde(default)]
    pub sources: Vec<SourceEntry>,
    #[serde(default)]
    pub wallhaven: WallhavenConfig,
    #[serde(default)]
    pub tray: TrayConfig,
    #[serde(default)]
    pub tui: TuiConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TuiConfig {
    #[serde(default)]
    pub key_profile: TuiKeyProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TuiKeyProfile {
    #[default]
    #[serde(alias = "default")]
    Emacs,
    Vim,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TrayConfig {
    #[serde(default)]
    pub accent: TrayAccent,
    #[serde(default)]
    pub autostart: TrayAutostartConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TrayAutostartConfig {
    /// Per-desktop login autostart for `walls-tray` (keys from [`crate::autostart::desktop_config_key`]).
    #[serde(default)]
    pub desktops: std::collections::HashMap<String, bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrayAccent {
    Blue,
    #[default]
    White,
    Cosmic,
    Green,
    Pink,
    Purple,
    Wallpaper,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "this serde struct mirrors the user-facing config schema, where separate toggles are clearer than a nested internal enum."
)]
pub struct ChangeConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub on_start: bool,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    #[serde(default = "default_true")]
    pub internet_enabled: bool,
    #[serde(default)]
    pub safe_mode: bool,
    #[serde(default)]
    pub change_lock_screen: bool,
    #[serde(default = "default_pref_ratio")]
    pub download_preference_ratio: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PathsConfig {
    pub cache_dir: String,
    pub download_dir: String,
    pub favorites_dir: String,
    pub fetched_dir: String,
    pub compose_dir: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QuotaConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_quota_mb")]
    pub size_mb: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SelectionConfig {
    #[serde(default = "default_true")]
    pub use_landscape_enabled: bool,
    #[serde(default = "default_avoid_recent")]
    pub avoid_recent: usize,
    #[serde(default = "default_refetch_below")]
    pub refetch_when_cache_below: usize,
    #[serde(default = "default_strategy")]
    pub strategy: SelectionStrategy,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionStrategy {
    Random,
    Sequential,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceEntry {
    pub enabled: bool,
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_path: Option<String>,
    /// Reddit listing sort (`hot`, `new`, `top`, `rising`, `controversial`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// Reddit time window for `top`/`controversial` (`hour`, `day`, `week`, `month`, `year`, `all`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Secrets {
    #[serde(default)]
    pub wallhaven_api_key: String,
    #[serde(default)]
    pub unsplash_access_key: String,
    /// Reddit API app client id (<https://www.reddit.com/prefs/apps> — script or installed app).
    #[serde(default)]
    pub reddit_client_id: String,
    /// Reddit API app secret (empty for installed-app type).
    #[serde(default)]
    pub reddit_client_secret: String,
}

impl Default for ChangeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            on_start: false,
            interval_secs: 300,
            internet_enabled: true,
            safe_mode: false,
            change_lock_screen: false,
            download_preference_ratio: 0.9,
        }
    }
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            size_mb: 1000,
        }
    }
}

impl Default for SelectionConfig {
    fn default() -> Self {
        Self {
            use_landscape_enabled: true,
            avoid_recent: 50,
            refetch_when_cache_below: 5,
            strategy: SelectionStrategy::Random,
        }
    }
}

/// Default configuration for a fresh install, seeded from `config.example.json` at repo root.
/// Extended source examples live in `config.sources.example.json` so first-run
/// config stays focused on immediately useful providers.
pub fn default_config() -> anyhow::Result<Config> {
    let mut config: Config = serde_json::from_str(include_str!("../../../../config.example.json"))?;
    config.wallhaven.search.atleast = wallhaven::detected_wallhaven_atleast()
        .unwrap_or(wallhaven::WALLHAVEN_FALLBACK_RESOLUTION)
        .into();
    Ok(config)
}

pub fn load_config(path: &Path) -> anyhow::Result<Config> {
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

/// Load config from disk, writing [`default_config`] first when the file is missing.
pub fn load_or_create_config(path: &Path) -> anyhow::Result<Config> {
    if path.exists() {
        return load_config(path);
    }
    let config = default_config()?;
    save_config_atomic(path, &config)?;
    tracing::info!("created default config at {}", path.display());
    Ok(config)
}

pub fn save_config_atomic(path: &Path, config: &Config) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}

/// Persist config and reconcile derived artifacts (tray autostart).
pub fn persist_config(path: &Path, config: &Config) -> anyhow::Result<()> {
    let mut config = config.clone();
    normalize_config_sources(&mut config.sources);
    save_config_atomic(path, &config)?;
    if let Err(err) = crate::autostart::sync_tray_autostart(&config) {
        tracing::warn!("tray autostart sync failed: {err:#}");
    }
    Ok(())
}

pub fn load_secrets(path: &Path) -> anyhow::Result<Secrets> {
    if !path.exists() {
        return Ok(Secrets::default());
    }
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

fn default_true() -> bool {
    true
}
fn default_interval() -> u64 {
    300
}
fn default_pref_ratio() -> f64 {
    0.9
}
fn default_quota_mb() -> u64 {
    1000
}
fn default_avoid_recent() -> usize {
    50
}
fn default_refetch_below() -> usize {
    5
}
fn default_strategy() -> SelectionStrategy {
    SelectionStrategy::Random
}

#[cfg(test)]
mod tests {
    use super::{
        load_config, save_config_atomic, Config, SelectionStrategy, SourceEntry, TuiKeyProfile,
    };

    fn test_config() -> Config {
        serde_json::from_value(serde_json::json!({
            "paths": {
                "cache_dir": "cache",
                "download_dir": "downloaded",
                "favorites_dir": "favorites",
                "fetched_dir": "fetched",
                "compose_dir": "wallpaper"
            }
        }))
        .expect("config")
    }

    #[test]
    fn load_or_create_config_writes_default_when_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");

        let loaded = super::load_or_create_config(&path).expect("create default config");

        assert!(path.is_file());
        assert!(loaded.change.enabled);
        assert_eq!(loaded.paths.compose_dir, "~/.local/share/walls/wallpaper");
        assert_eq!(loaded.wallhaven.search.q, super::WALLHAVEN_DEFAULT_QUERY);
        assert!(super::wallhaven_resolution_supported(
            &loaded.wallhaven.search.atleast
        ));
    }

    #[test]
    fn save_config_atomic_writes_pretty_json() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");
        let mut config = test_config();
        config.change.enabled = false;
        config.selection.strategy = SelectionStrategy::Sequential;

        save_config_atomic(&path, &config).expect("save config");

        let text = std::fs::read_to_string(&path).expect("read config");
        assert!(text.ends_with('\n'), "{text}");
        assert!(text.contains("\"enabled\": false"), "{text}");
        assert!(text.contains("\"strategy\": \"sequential\""), "{text}");
        let loaded = load_config(&path).expect("load config");
        assert!(!loaded.change.enabled);
        assert_eq!(loaded.selection.strategy, SelectionStrategy::Sequential);
    }

    #[test]
    fn tui_key_profile_defaults_and_round_trips() {
        let mut config = test_config();
        assert_eq!(config.tui.key_profile, TuiKeyProfile::Emacs);
        config.tui.key_profile = TuiKeyProfile::Vim;

        let value = serde_json::to_value(&config).expect("serialize config");
        assert_eq!(value["tui"]["key_profile"], "vim");

        let loaded: Config = serde_json::from_value(value).expect("deserialize config");
        assert_eq!(loaded.tui.key_profile, TuiKeyProfile::Vim);

        let legacy: Config = serde_json::from_str(
            r#"{
                "paths": {
                    "cache_dir": "/tmp/cache",
                    "download_dir": "/tmp/downloads",
                    "favorites_dir": "/tmp/favorites",
                    "fetched_dir": "/tmp/fetched",
                    "compose_dir": "/tmp/compose"
                },
                "tui": { "key_profile": "default" }
            }"#,
        )
        .expect("deserialize legacy default key profile");
        assert_eq!(legacy.tui.key_profile, TuiKeyProfile::Emacs);
    }

    #[test]
    fn persist_config_strips_unrelated_source_fields() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");
        let mut config = test_config();
        config.sources.push(SourceEntry {
            enabled: true,
            source_type: "reddit".into(),
            query: Some("wallpapers".into()),
            sort: Some("hot".into()),
            path: Some("/should-drop".into()),
            api_key: Some("nope".into()),
            label: None,
            url: None,
            collection: None,
            user: None,
            topic: None,
            orientation: None,
            image_path: None,
            title_path: None,
            time: None,
        });

        super::persist_config(&path, &config).expect("persist");

        let loaded = load_config(&path).expect("load");
        let reddit = &loaded.sources[0];
        assert_eq!(reddit.source_type, "reddit");
        assert!(reddit.path.is_none());
        assert!(reddit.api_key.is_none());
        assert_eq!(reddit.query.as_deref(), Some("wallpapers"));
    }

    #[test]
    fn failed_atomic_save_does_not_replace_existing_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");
        std::fs::write(&path, "original").expect("write original");
        std::fs::create_dir(path.with_extension("json.tmp")).expect("tmp dir");

        let config = test_config();

        let err = save_config_atomic(&path, &config).expect_err("save should fail");
        assert!(
            err.to_string().contains("Is a directory")
                || err.to_string().contains("is a directory"),
            "{err}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("read original"),
            "original"
        );
    }
}
