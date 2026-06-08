//! Read COSMIC desktop accent colours from theme config files.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::SystemTime;

use regex::Regex;

use crate::tray_icon::TrayAccentPalette;

static ENV_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Serialize tests that mutate process environment variables.
pub fn lock_env_for_tests() -> MutexGuard<'static, ()> {
    ENV_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Back-compat alias for older call sites.
pub fn lock_xdg_config_home_for_tests() -> MutexGuard<'static, ()> {
    lock_env_for_tests()
}

const COSMIC_THEME_MODE: &str = "com.system76.CosmicTheme.Mode/v1/is_dark";
const COSMIC_THEME_DARK_ACCENT: &str = "com.system76.CosmicTheme.Dark/v1/accent";
const COSMIC_THEME_LIGHT_ACCENT: &str = "com.system76.CosmicTheme.Light/v1/accent";

/// COSMIC config root (`$XDG_CONFIG_HOME/cosmic` or `~/.config/cosmic`).
pub fn cosmic_config_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("cosmic"));
    }
    dirs::home_dir().map(|home| home.join(".config/cosmic"))
}

/// Build a tray palette from the active COSMIC accent theme.
pub fn cosmic_accent_palette() -> Option<TrayAccentPalette> {
    let base = cosmic_config_dir()?;
    let is_dark = read_is_dark(&base)?;
    let accent_path = accent_file_path(&base, is_dark);
    let contents = fs::read_to_string(accent_path).ok()?;
    palette_from_accent_ron(&contents)
}

/// Latest modification time of COSMIC mode/accent files, for tray refresh polling.
pub fn cosmic_theme_stamp() -> Option<SystemTime> {
    let base = cosmic_config_dir()?;
    let mode_path = base.join(COSMIC_THEME_MODE);
    let accent_path = accent_file_path(&base, read_is_dark(&base)?);
    let mode_mtime = fs::metadata(mode_path).ok()?.modified().ok()?;
    let accent_mtime = fs::metadata(accent_path).ok()?.modified().ok()?;
    Some(mode_mtime.max(accent_mtime))
}

fn read_is_dark(base: &Path) -> Option<bool> {
    let contents = fs::read_to_string(base.join(COSMIC_THEME_MODE)).ok()?;
    match contents.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn accent_file_path(base: &Path, is_dark: bool) -> PathBuf {
    if is_dark {
        base.join(COSMIC_THEME_DARK_ACCENT)
    } else {
        base.join(COSMIC_THEME_LIGHT_ACCENT)
    }
}

fn palette_from_accent_ron(contents: &str) -> Option<TrayAccentPalette> {
    let primary = parse_ron_color(contents, "base")?;
    let secondary =
        parse_ron_color(contents, "hover").or_else(|| parse_ron_color(contents, "selected"))?;
    let highlight = parse_ron_color(contents, "focus").unwrap_or_else(|| scale_rgb(primary, 1.15));
    let border = parse_ron_color(contents, "border").unwrap_or(primary);
    Some(TrayAccentPalette {
        primary,
        secondary,
        highlight,
        border,
    })
}

fn parse_ron_color(contents: &str, field: &str) -> Option<[u8; 3]> {
    let re = Regex::new(&format!(
        r"{field}:\s*\(\s*red:\s*(-?\d+(?:\.\d+)?),\s*green:\s*(-?\d+(?:\.\d+)?),\s*blue:\s*(-?\d+(?:\.\d+)?)"
    ))
    .ok()?;
    let caps = re.captures(contents)?;
    let r = caps.get(1)?.as_str().parse::<f32>().ok()?;
    let g = caps.get(2)?.as_str().parse::<f32>().ok()?;
    let b = caps.get(3)?.as_str().parse::<f32>().ok()?;
    Some(float_rgb_to_bytes(r, g, b))
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "COSMIC theme channels are clamped to valid RGB bytes."
)]
fn float_rgb_to_bytes(r: f32, g: f32, b: f32) -> [u8; 3] {
    [
        (r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (b.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "palette scaling clamps to valid RGB bytes."
)]
fn scale_rgb(rgb: [u8; 3], factor: f32) -> [u8; 3] {
    rgb.map(|channel| (f32::from(channel) * factor).clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    const SAMPLE_ACCENT: &str = r"(
    base: (
        red: 0.7176471,
        green: 0.7411765,
        blue: 0.972549,
        alpha: 1.0,
    ),
    hover: (
        red: 0.6518322,
        green: 0.6706558,
        blue: 0.8557538,
        alpha: 1.0,
    ),
    focus: (
        red: 0.7176471,
        green: 0.7411765,
        blue: 0.972549,
        alpha: 1.0,
    ),
    border: (
        red: 0.5,
        green: 0.6,
        blue: 0.7,
        alpha: 1.0,
    ),
)";

    #[test]
    fn palette_from_accent_ron_maps_cosmic_roles() {
        let palette = palette_from_accent_ron(SAMPLE_ACCENT).expect("palette");
        assert_eq!(palette.primary, [183, 189, 248]);
        assert_eq!(palette.secondary, [166, 171, 218]);
        assert_eq!(palette.highlight, [183, 189, 248]);
        assert_eq!(palette.border, [128, 153, 179]);
    }

    #[test]
    fn cosmic_theme_stamp_tracks_accent_file_updates() {
        let _lock = super::lock_xdg_config_home_for_tests();
        let root = tempfile::tempdir().unwrap();
        let cosmic_root = root.path().join("cosmic");
        fs::create_dir_all(cosmic_root.join("com.system76.CosmicTheme.Mode/v1")).unwrap();
        fs::create_dir_all(cosmic_root.join("com.system76.CosmicTheme.Dark/v1")).unwrap();
        fs::write(cosmic_root.join(COSMIC_THEME_MODE), "true").unwrap();
        fs::write(cosmic_root.join(COSMIC_THEME_DARK_ACCENT), SAMPLE_ACCENT).unwrap();

        std::env::set_var("XDG_CONFIG_HOME", root.path());

        let first = cosmic_theme_stamp().expect("stamp");
        thread::sleep(Duration::from_millis(20));
        fs::write(
            cosmic_root.join(COSMIC_THEME_DARK_ACCENT),
            SAMPLE_ACCENT.replace("0.5", "0.55"),
        )
        .unwrap();
        let second = cosmic_theme_stamp().expect("stamp");

        assert!(second > first);

        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
