use serde::{Deserialize, Serialize};

/// How the desktop should scale the wallpaper (Variety `set_wallpaper` arg 4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FillMode {
    #[default]
    Os,
    Zoom,
    Spanned,
    Centered,
    Scaled,
    Stretched,
    Wallpaper,
}

impl FillMode {
    pub fn from_display_mode(mode: &str) -> Self {
        match mode {
            "zoom" | "smart" | "fill-with-black" | "fill-with-blur" => FillMode::Zoom,
            "spanned" => FillMode::Spanned,
            "gnome-centered" | "centered" => FillMode::Centered,
            "gnome-scaled" | "scaled" => FillMode::Scaled,
            "gnome-stretched" | "stretched" => FillMode::Stretched,
            "gnome-spanned" | "gnome-wallpaper" | "wallpaper" => FillMode::Spanned,
            _ => FillMode::Os,
        }
    }

    /// GNOME `picture-options` value when applicable.
    pub fn gnome_picture_options(self) -> Option<&'static str> {
        match self {
            FillMode::Os => None,
            FillMode::Zoom => Some("zoom"),
            FillMode::Spanned => Some("spanned"),
            FillMode::Centered => Some("centered"),
            FillMode::Scaled => Some("scaled"),
            FillMode::Stretched => Some("stretched"),
            FillMode::Wallpaper => Some("wallpaper"),
        }
    }
}

/// Why the wallpaper is being applied (Variety arg 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyTrigger {
    Auto,
    Manual,
    Refresh,
}

impl ApplyTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            ApplyTrigger::Auto => "auto",
            ApplyTrigger::Manual => "manual",
            ApplyTrigger::Refresh => "refresh",
        }
    }
}
