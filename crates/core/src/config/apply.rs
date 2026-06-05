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

fn default_backend_auto() -> ApplyBackendSetting {
    ApplyBackendSetting::Auto
}
fn default_cosmic_method() -> CosmicMethod {
    CosmicMethod::CosmicConfig
}
fn default_cosmic_config_path() -> String {
    "~/.config/cosmic/com.system76.CosmicBackground/v1/all".into()
}
