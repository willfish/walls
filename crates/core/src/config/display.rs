use serde::{Deserialize, Serialize};

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

fn default_display_mode() -> String {
    "os".into()
}
fn default_imagemagick_command() -> String {
    "magick".into()
}
