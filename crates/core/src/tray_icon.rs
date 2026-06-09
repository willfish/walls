//! Tray icon accent colours and wallpaper-derived palettes.

use std::path::Path;

use crate::apply::{detect_desktop, Desktop};
use crate::config::{Config, TrayAccent};
use crate::cosmic_theme;
use image::imageops::FilterType;

/// RGB accent palette for the brand tray SVG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayAccentPalette {
    pub primary: [u8; 3],
    pub secondary: [u8; 3],
    pub highlight: [u8; 3],
    pub border: [u8; 3],
}

impl TrayAccentPalette {
    pub fn blue() -> Self {
        Self {
            primary: [0x4A, 0x90, 0xD9],
            secondary: [0x5B, 0x9A, 0xE8],
            highlight: [0xA8, 0xCF, 0xFF],
            border: [140, 190, 255],
        }
    }

    pub fn green() -> Self {
        Self {
            primary: [0x3D, 0xA8, 0x6E],
            secondary: [0x4E, 0xC4, 0x8A],
            highlight: [0x9F, 0xE8, 0xC0],
            border: [120, 220, 160],
        }
    }

    pub fn pink() -> Self {
        Self {
            primary: [0xD9, 0x5A, 0x9A],
            secondary: [0xE8, 0x7B, 0xB0],
            highlight: [0xFF, 0xC8, 0xE0],
            border: [255, 160, 200],
        }
    }

    pub fn purple() -> Self {
        Self {
            primary: [0x8A, 0x5A, 0xD9],
            secondary: [0xA0, 0x7B, 0xE8],
            highlight: [0xD0, 0xB8, 0xFF],
            border: [190, 160, 255],
        }
    }

    /// Monochrome white palette for dark panels (COSMIC symbolic tray icon style).
    pub fn white() -> Self {
        Self {
            primary: [255, 255, 255],
            secondary: [210, 210, 218],
            highlight: [255, 255, 255],
            border: [235, 235, 242],
        }
    }

    pub fn from_accent(accent: TrayAccent) -> Self {
        match accent {
            TrayAccent::White => Self::white(),
            TrayAccent::Green => Self::green(),
            TrayAccent::Pink => Self::pink(),
            TrayAccent::Purple => Self::purple(),
            TrayAccent::Blue | TrayAccent::Wallpaper | TrayAccent::Cosmic => Self::blue(),
        }
    }

    pub fn from_dominant(rgb: [u8; 3]) -> Self {
        Self {
            primary: scale_rgb(rgb, 0.82),
            secondary: rgb,
            highlight: scale_rgb(rgb, 1.28),
            border: scale_rgb(rgb, 1.12),
        }
    }
}

/// Whether the current session is COSMIC (`XDG_CURRENT_DESKTOP=COSMIC`).
pub fn is_cosmic_session() -> bool {
    detect_desktop() == Desktop::Cosmic
}

/// Whether `accent` can be selected on this session.
pub fn tray_accent_available(accent: TrayAccent) -> bool {
    match accent {
        TrayAccent::Cosmic => is_cosmic_session(),
        _ => true,
    }
}

/// Config accent with session-specific options applied.
pub fn effective_tray_accent(accent: TrayAccent) -> TrayAccent {
    if tray_accent_available(accent) {
        accent
    } else {
        TrayAccent::White
    }
}

/// Resolve the tray palette from config and optional composed wallpaper path.
pub fn resolve_tray_palette(config: &Config, wallpaper_path: Option<&Path>) -> TrayAccentPalette {
    match effective_tray_accent(config.tray.accent) {
        TrayAccent::Wallpaper => wallpaper_path
            .and_then(dominant_color_from_image)
            .map_or_else(TrayAccentPalette::white, TrayAccentPalette::from_dominant),
        TrayAccent::Cosmic => {
            cosmic_theme::cosmic_accent_palette().unwrap_or_else(TrayAccentPalette::white)
        }
        accent => TrayAccentPalette::from_accent(accent),
    }
}

/// User-facing label for config display and TUI choices.
pub fn tray_accent_label(accent: TrayAccent) -> &'static str {
    match accent {
        TrayAccent::Blue => "blue",
        TrayAccent::White => "white",
        TrayAccent::Cosmic => "cosmic",
        TrayAccent::Green => "green",
        TrayAccent::Pink => "pink",
        TrayAccent::Purple => "purple",
        TrayAccent::Wallpaper => "wallpaper",
    }
}

/// Parse a config/TUI value into [`TrayAccent`].
pub fn parse_tray_accent(value: &str) -> Option<TrayAccent> {
    match value.trim().to_ascii_lowercase().as_str() {
        "blue" => Some(TrayAccent::Blue),
        "white" => Some(TrayAccent::White),
        "cosmic" => Some(TrayAccent::Cosmic),
        "green" => Some(TrayAccent::Green),
        "pink" => Some(TrayAccent::Pink),
        "purple" => Some(TrayAccent::Purple),
        "wallpaper" => Some(TrayAccent::Wallpaper),
        _ => None,
    }
}

const TRAY_ACCENT_CHOICES_BASE: &[&str] =
    &["white", "blue", "green", "pink", "purple", "wallpaper"];
const TRAY_ACCENT_CHOICES_WITH_COSMIC: &[&str] = &[
    "white",
    "blue",
    "cosmic",
    "green",
    "pink",
    "purple",
    "wallpaper",
];

/// Tray accent values offered in config editors on this session.
pub fn tray_accent_choices() -> &'static [&'static str] {
    if is_cosmic_session() {
        TRAY_ACCENT_CHOICES_WITH_COSMIC
    } else {
        TRAY_ACCENT_CHOICES_BASE
    }
}

/// Canonical blue hex values in `assets/icons/walls-tray.svg` (desktop launcher uses the same file).
pub const BRAND_PRIMARY_HEX: &str = "#4A90D9";
pub const BRAND_SECONDARY_HEX: &str = "#5B9AE8";
pub const BRAND_HIGHLIGHT_HEX: &str = "#A8CFFF";

/// Tint the embedded brand SVG blues to `palette` before rasterization.
pub fn tint_brand_svg(svg: &str, palette: &TrayAccentPalette) -> String {
    svg.replace(BRAND_PRIMARY_HEX, &rgb_hex(palette.primary))
        .replace(BRAND_SECONDARY_HEX, &rgb_hex(palette.secondary))
        .replace(BRAND_HIGHLIGHT_HEX, &rgb_hex(palette.highlight))
}

/// Weighted average of saturated mid-tone pixels from a downsampled image.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "colour averages are intentionally clamped to byte channels."
)]
pub fn dominant_color_from_image(path: &Path) -> Option<[u8; 3]> {
    let img = image::open(path).ok()?;
    let thumb = img.resize_exact(16, 16, FilterType::Triangle).to_rgba8();
    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;
    let mut weight = 0u64;

    for pixel in thumb.pixels() {
        if pixel[3] < 128 {
            continue;
        }
        let r = f32::from(pixel[0]);
        let g = f32::from(pixel[1]);
        let b = f32::from(pixel[2]);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lum = f32::midpoint(max, min);
        if !(20.0..235.0).contains(&lum) {
            continue;
        }
        let sat = if max <= 0.0 { 0.0 } else { (max - min) / max };
        let w = ((sat * 2.0) + 0.5).max(0.1);
        let w_u = (w * 100.0).round() as u64;
        r_sum += u64::from(pixel[0]) * w_u;
        g_sum += u64::from(pixel[1]) * w_u;
        b_sum += u64::from(pixel[2]) * w_u;
        weight += w_u;
    }

    if weight == 0 {
        return None;
    }

    Some([
        (r_sum / weight) as u8,
        (g_sum / weight) as u8,
        (b_sum / weight) as u8,
    ])
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "palette scaling clamps to valid RGB bytes."
)]
fn scale_rgb(rgb: [u8; 3], factor: f32) -> [u8; 3] {
    rgb.map(|channel| (f32::from(channel) * factor).clamp(0.0, 255.0) as u8)
}

fn rgb_hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn parse_tray_accent_accepts_config_values() {
        assert_eq!(parse_tray_accent("Blue"), Some(TrayAccent::Blue));
        assert_eq!(parse_tray_accent("white"), Some(TrayAccent::White));
        assert_eq!(parse_tray_accent("cosmic"), Some(TrayAccent::Cosmic));
        assert_eq!(parse_tray_accent("wallpaper"), Some(TrayAccent::Wallpaper));
        assert_eq!(parse_tray_accent("magenta"), None);
    }

    #[test]
    fn blue_palette_matches_canonical_brand_svg_hex() {
        let palette = TrayAccentPalette::blue();
        assert_eq!(rgb_hex(palette.primary), BRAND_PRIMARY_HEX);
        assert_eq!(rgb_hex(palette.secondary), BRAND_SECONDARY_HEX);
        assert_eq!(rgb_hex(palette.highlight), BRAND_HIGHLIGHT_HEX);
    }

    #[test]
    fn white_palette_tints_to_monochrome() {
        let svg = "<path fill=\"#5B9AE8\"/><circle fill=\"#A8CFFF\"/><rect stroke=\"#4A90D9\"/>";
        let tinted = tint_brand_svg(svg, &TrayAccentPalette::white());
        assert!(tinted.contains("#FFFFFF"));
        assert!(tinted.contains("#D2D2DA"));
        assert!(!tinted.contains("#4A90D9"));
    }

    #[test]
    fn tint_brand_svg_replaces_blue_swatches() {
        let svg = "<path fill=\"#5B9AE8\"/><circle fill=\"#A8CFFF\"/><rect stroke=\"#4A90D9\"/>";
        let tinted = tint_brand_svg(svg, &TrayAccentPalette::pink());
        assert!(tinted.contains("#D95A9A"));
        assert!(tinted.contains("#E87BB0"));
        assert!(tinted.contains("#FFC8E0"));
        assert!(!tinted.contains("#4A90D9"));
    }

    #[test]
    fn dominant_color_from_solid_green_image() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("green.png");
        let image = ImageBuffer::from_pixel(8, 8, Rgba([20u8, 180, 40, 255]));
        image.save(&path).unwrap();

        let rgb = dominant_color_from_image(&path).expect("dominant color");
        assert!(rgb[1] > rgb[0], "green channel should dominate: {rgb:?}");
        assert!(rgb[1] > rgb[2], "green channel should dominate: {rgb:?}");
    }

    #[test]
    fn tray_accent_choices_match_cosmic_session() {
        let choices = tray_accent_choices();
        if is_cosmic_session() {
            assert!(choices.contains(&"cosmic"));
        } else {
            assert!(!choices.contains(&"cosmic"));
        }
    }

    #[test]
    fn effective_tray_accent_falls_back_off_cosmic_session() {
        if is_cosmic_session() {
            assert_eq!(
                effective_tray_accent(TrayAccent::Cosmic),
                TrayAccent::Cosmic
            );
        } else {
            assert_eq!(effective_tray_accent(TrayAccent::Cosmic), TrayAccent::White);
        }
    }

    #[test]
    fn wallpaper_accent_defaults_to_white_without_image() {
        let config = Config {
            tray: crate::config::TrayConfig {
                accent: TrayAccent::Wallpaper,
                ..crate::config::TrayConfig::default()
            },
            ..crate::config::default_config().expect("default config")
        };
        let palette = resolve_tray_palette(&config, None);
        assert_eq!(palette.primary, TrayAccentPalette::white().primary);
    }
}
