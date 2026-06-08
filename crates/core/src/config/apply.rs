use serde::{Deserialize, Serialize};

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

/// Fields walls writes into the COSMIC `all` background entry on each apply.
///
/// COSMIC uses this file for its own slideshow (`rotation_frequency` > 0). walls sets
/// `rotation_frequency: 0` by default so only walls' scheduler rotates wallpapers.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CosmicBackgroundEntryConfig {
    #[serde(default = "default_cosmic_rotation_frequency")]
    pub rotation_frequency: u64,
    #[serde(default)]
    pub filter_by_theme: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CosmicApplyConfig {
    #[serde(default = "default_cosmic_method")]
    pub method: CosmicMethod,
    #[serde(default = "default_cosmic_config_path")]
    pub config_path: String,
    #[serde(default)]
    pub use_original_path: bool,
    #[serde(default)]
    pub entry: CosmicBackgroundEntryConfig,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CosmicMethod {
    CosmicConfig,
    CosmicExtBgCtl,
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

impl Default for CosmicBackgroundEntryConfig {
    fn default() -> Self {
        Self {
            rotation_frequency: default_cosmic_rotation_frequency(),
            filter_by_theme: false,
        }
    }
}

impl Default for CosmicApplyConfig {
    fn default() -> Self {
        Self {
            method: CosmicMethod::CosmicConfig,
            config_path: default_cosmic_config_path(),
            use_original_path: false,
            entry: CosmicBackgroundEntryConfig::default(),
        }
    }
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
fn default_cosmic_rotation_frequency() -> u64 {
    0
}
