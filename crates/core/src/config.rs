use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

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
pub struct ApplyConfig {
    #[serde(default = "default_backend_auto")]
    pub backend: ApplyBackendSetting,
    #[serde(default)]
    pub cosmic: CosmicApplyConfig,
    #[serde(default)]
    pub custom_script: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ApplyBackendSetting {
    Auto,
    Cosmic,
    CosmicExtBgCtl,
    Gnome,
    Kde,
    Xfce,
    Sway,
    Wlroots,
    Hyprland,
    Feh,
    CustomScript,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CosmicApplyConfig {
    #[serde(default = "default_cosmic_method")]
    pub method: CosmicMethod,
    #[serde(default = "default_cosmic_config_path")]
    pub config_path: String,
    #[serde(default)]
    pub use_original_path: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CosmicMethod {
    CosmicConfig,
    CosmicExtBgCtl,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayConfig {
    #[serde(default = "default_display_mode")]
    pub mode: String,
    #[serde(default)]
    pub auto_rotate: bool,
    #[serde(default = "default_imagemagick_command")]
    pub imagemagick_command: String,
    #[serde(default)]
    pub target_width: Option<u32>,
    #[serde(default)]
    pub target_height: Option<u32>,
    #[serde(default)]
    pub filters: DisplayFiltersConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayFiltersConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_imagemagick_command")]
    pub command: String,
    #[serde(default)]
    pub filters: Vec<ImageMagickFilterConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageMagickFilterConfig {
    pub name: String,
    #[serde(default)]
    pub args: Vec<String>,
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
pub struct WallhavenConfig {
    #[serde(default)]
    pub collections: Vec<WallhavenCollection>,
    #[serde(default)]
    pub search: WallhavenSearch,
    #[serde(default = "default_prefer")]
    pub prefer: WallhavenPrefer,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WallhavenCollection {
    pub username: String,
    pub id: u32,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WallhavenSearch {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_categories")]
    pub categories: String,
    #[serde(default = "default_purity")]
    pub purity: String,
    #[serde(default = "default_sorting")]
    pub sorting: String,
    #[serde(default = "default_order")]
    pub order: String,
    #[serde(default = "default_atleast")]
    pub atleast: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WallhavenPrefer {
    CollectionsThenSearch,
    SearchOnly,
    CollectionsOnly,
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

impl Default for ApplyConfig {
    fn default() -> Self {
        Self {
            backend: ApplyBackendSetting::Auto,
            cosmic: CosmicApplyConfig::default(),
            custom_script: None,
        }
    }
}

impl Default for CosmicApplyConfig {
    fn default() -> Self {
        Self {
            method: CosmicMethod::CosmicConfig,
            config_path: default_cosmic_config_path(),
            use_original_path: false,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            mode: default_display_mode(),
            auto_rotate: false,
            imagemagick_command: default_imagemagick_command(),
            target_width: None,
            target_height: None,
            filters: DisplayFiltersConfig::default(),
        }
    }
}

impl Default for DisplayFiltersConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            command: default_imagemagick_command(),
            filters: Vec::new(),
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

impl Default for WallhavenConfig {
    fn default() -> Self {
        Self {
            collections: Vec::new(),
            search: WallhavenSearch::default(),
            prefer: WallhavenPrefer::CollectionsThenSearch,
        }
    }
}

impl Default for WallhavenSearch {
    fn default() -> Self {
        Self {
            q: String::new(),
            categories: default_categories(),
            purity: default_purity(),
            sorting: default_sorting(),
            order: default_order(),
            atleast: default_atleast(),
        }
    }
}

pub fn load_config(path: &Path) -> anyhow::Result<Config> {
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
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
fn default_backend_auto() -> ApplyBackendSetting {
    ApplyBackendSetting::Auto
}
fn default_cosmic_method() -> CosmicMethod {
    CosmicMethod::CosmicConfig
}
fn default_cosmic_config_path() -> String {
    "~/.config/cosmic/com.system76.CosmicBackground/v1/all".into()
}
fn default_display_mode() -> String {
    "os".into()
}

fn default_imagemagick_command() -> String {
    "magick".into()
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
fn default_prefer() -> WallhavenPrefer {
    WallhavenPrefer::CollectionsThenSearch
}
fn default_categories() -> String {
    "111".into()
}
fn default_purity() -> String {
    "100".into()
}
fn default_sorting() -> String {
    "random".into()
}
fn default_order() -> String {
    "desc".into()
}
fn default_atleast() -> String {
    "1920x1080".into()
}
