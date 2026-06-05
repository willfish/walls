use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

mod apply;
mod display;
mod wallhaven;

pub use apply::{ApplyBackendSetting, ApplyConfig, CosmicApplyConfig, CosmicMethod};
pub use display::{DisplayConfig, DisplayFiltersConfig, ImageMagickFilterConfig};
pub use wallhaven::{WallhavenCollection, WallhavenConfig, WallhavenPrefer, WallhavenSearch};

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
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Secrets {
    #[serde(default)]
    pub wallhaven_api_key: String,
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

pub fn load_config(path: &Path) -> anyhow::Result<Config> {
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
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

pub fn load_secrets(path: &Path) -> anyhow::Result<Secrets> {
    if !path.exists() {
        return Ok(Secrets {
            wallhaven_api_key: String::new(),
        });
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
    use super::{load_config, save_config_atomic, Config, SelectionStrategy};

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
