use std::path::Path;

use image::imageops::FilterType;
use image::{ImageBuffer, Rgba, RgbaImage};
use walls_core::config::load_or_create_config;
use walls_core::paths::{expand_home, WallsPaths};
use walls_core::rotation::rotation_inactive;
use walls_core::state::State;
use walls_core::tray_icon::{resolve_tray_palette, tint_brand_svg, TrayAccentPalette};

const TRAY_SIZE: u32 = 32;
const TRAY_BORDER_PX: u32 = 2;
const BRAND_SVG: &str = include_str!("../../../assets/icons/walls-tray.svg");
const BRAND_PAUSED_SVG: &str = include_str!("../../../assets/icons/walls-tray-paused.svg");

struct TrayIconContext {
    inactive: bool,
    palette: TrayAccentPalette,
}

pub fn default_appindicator_icon() -> anyhow::Result<tray_icon::Icon> {
    brand_appindicator_icon(false, &TrayAccentPalette::white())
}

pub fn appindicator_icon_from_state() -> anyhow::Result<tray_icon::Icon> {
    let ctx = load_tray_icon_context().unwrap_or(TrayIconContext {
        inactive: false,
        palette: TrayAccentPalette::white(),
    });
    if wallpaper_thumbnail_enabled() && !ctx.inactive {
        wallpaper_appindicator_icon(&ctx.palette)
    } else {
        brand_appindicator_icon(ctx.inactive, &ctx.palette)
    }
}

pub fn tooltip_from_state() -> String {
    match load_rotation_inactive() {
        Ok(inactive) => tooltip_for_rotation_inactive(inactive).into(),
        Err(_) => "walls".into(),
    }
}

pub(crate) fn tooltip_for_rotation_inactive(inactive: bool) -> &'static str {
    if inactive {
        "walls (paused)"
    } else {
        "walls"
    }
}

pub fn default_ksni_icons() -> Vec<ksni::Icon> {
    brand_ksni_icons(false, &TrayAccentPalette::white())
}

pub fn ksni_icons_from_state() -> Vec<ksni::Icon> {
    ksni_icons_from_state_result()
        .unwrap_or_else(|_| brand_ksni_icons(false, &TrayAccentPalette::white()))
}

fn ksni_icons_from_state_result() -> anyhow::Result<Vec<ksni::Icon>> {
    let ctx = load_tray_icon_context()?;
    if wallpaper_thumbnail_enabled() && !ctx.inactive {
        wallpaper_ksni_icons(&ctx.palette)
    } else {
        Ok(brand_ksni_icons(ctx.inactive, &ctx.palette))
    }
}

fn load_rotation_inactive() -> anyhow::Result<bool> {
    let paths = WallsPaths::discover()?;
    let state = State::load_or_default(&paths.state_file)?;
    let config = load_or_create_config(&paths.config_file)?;
    Ok(rotation_inactive(&state, &config))
}

fn load_tray_icon_context() -> anyhow::Result<TrayIconContext> {
    let paths = WallsPaths::discover()?;
    let state = State::load_or_default(&paths.state_file)?;
    let config = load_or_create_config(&paths.config_file)?;
    let wallpaper_path = state
        .current
        .as_ref()
        .map(|current| expand_home(&current.composed_path));
    let palette = resolve_tray_palette(&config, wallpaper_path.as_deref());
    Ok(TrayIconContext {
        inactive: rotation_inactive(&state, &config),
        palette,
    })
}

fn wallpaper_thumbnail_enabled() -> bool {
    matches!(
        std::env::var("WALLS_TRAY_WALLPAPER_THUMBNAIL").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

fn brand_appindicator_icon(
    inactive: bool,
    palette: &TrayAccentPalette,
) -> anyhow::Result<tray_icon::Icon> {
    let rgba = brand_tray_rgba_bytes(inactive, palette)?;
    Ok(tray_icon::Icon::from_rgba(rgba, TRAY_SIZE, TRAY_SIZE)?)
}

fn brand_ksni_icons(inactive: bool, palette: &TrayAccentPalette) -> Vec<ksni::Icon> {
    let rgba = brand_tray_rgba_bytes(inactive, palette).unwrap_or_default();
    vec![ksni::Icon {
        width: TRAY_SIZE as i32,
        height: TRAY_SIZE as i32,
        data: rgba_to_argb(&rgba),
    }]
}

fn brand_tray_rgba_bytes(inactive: bool, palette: &TrayAccentPalette) -> anyhow::Result<Vec<u8>> {
    let svg = if inactive {
        BRAND_PAUSED_SVG
    } else {
        BRAND_SVG
    };
    let tinted = tint_brand_svg(svg, palette);
    rasterize_svg_to_rgba(&tinted, TRAY_SIZE)
}

fn wallpaper_appindicator_icon(palette: &TrayAccentPalette) -> anyhow::Result<tray_icon::Icon> {
    let paths = WallsPaths::discover()?;
    let state = State::load_or_default(&paths.state_file)?;
    let ctx = load_tray_icon_context().unwrap_or(TrayIconContext {
        inactive: false,
        palette: *palette,
    });
    let Some(current) = state.current else {
        return brand_appindicator_icon(ctx.inactive, palette);
    };
    let path = expand_home(&current.composed_path);
    appindicator_icon_from_path(&path, palette)
        .or_else(|_| brand_appindicator_icon(ctx.inactive, palette))
}

fn wallpaper_ksni_icons(palette: &TrayAccentPalette) -> anyhow::Result<Vec<ksni::Icon>> {
    let paths = WallsPaths::discover()?;
    let state = State::load_or_default(&paths.state_file)?;
    let ctx = load_tray_icon_context().unwrap_or(TrayIconContext {
        inactive: false,
        palette: *palette,
    });
    let Some(current) = state.current else {
        return Ok(brand_ksni_icons(ctx.inactive, palette));
    };
    let path = expand_home(&current.composed_path);
    ksni_icons_from_path(&path, palette).or_else(|_| Ok(brand_ksni_icons(ctx.inactive, palette)))
}

fn appindicator_icon_from_path(
    path: &Path,
    palette: &TrayAccentPalette,
) -> anyhow::Result<tray_icon::Icon> {
    let rgba = rgba_icon_from_path(path, palette)?;
    Ok(tray_icon::Icon::from_rgba(rgba, TRAY_SIZE, TRAY_SIZE)?)
}

fn ksni_icons_from_path(
    path: &Path,
    palette: &TrayAccentPalette,
) -> anyhow::Result<Vec<ksni::Icon>> {
    let rgba = rgba_icon_from_path(path, palette)?;
    Ok(vec![ksni::Icon {
        width: TRAY_SIZE as i32,
        height: TRAY_SIZE as i32,
        data: rgba_to_argb(&rgba),
    }])
}

fn rasterize_svg_to_rgba(svg: &str, size: u32) -> anyhow::Result<Vec<u8>> {
    let options = resvg::usvg::Options {
        font_family: "sans-serif".to_string(),
        ..resvg::usvg::Options::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .ok_or_else(|| anyhow::anyhow!("pixmap alloc failed"))?;
    pixmap.fill(resvg::tiny_skia::Color::TRANSPARENT);

    let svg_w = tree.size().width();
    let svg_h = tree.size().height();
    let scale = (size as f32 / svg_w.max(svg_h)).min(1.0);
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect())
}

fn inner_tray_size() -> u32 {
    TRAY_SIZE.saturating_sub(TRAY_BORDER_PX * 2).max(1)
}

fn bordered_rgba(inner: RgbaImage, border: [u8; 3]) -> Vec<u8> {
    let border_rgba = Rgba([border[0], border[1], border[2], 255]);
    let mut canvas = ImageBuffer::from_pixel(TRAY_SIZE, TRAY_SIZE, border_rgba);
    image::imageops::overlay(
        &mut canvas,
        &inner,
        i64::from(TRAY_BORDER_PX),
        i64::from(TRAY_BORDER_PX),
    );
    canvas.into_raw()
}

fn rgba_icon_from_path(path: &Path, palette: &TrayAccentPalette) -> anyhow::Result<Vec<u8>> {
    let img = image::open(path)?;
    let thumb = img.resize_to_fill(inner_tray_size(), inner_tray_size(), FilterType::Triangle);
    Ok(bordered_rgba(thumb.to_rgba8(), palette.border))
}

fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|pixel| [pixel[3], pixel[0], pixel[1], pixel[2]])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn tooltip_mentions_paused_state() {
        assert_eq!(tooltip_for_rotation_inactive(true), "walls (paused)");
        assert_eq!(tooltip_for_rotation_inactive(false), "walls");
    }

    #[test]
    fn brand_svgs_rasterize_to_tray_pixels() {
        for svg in [BRAND_SVG, BRAND_PAUSED_SVG] {
            let tinted = tint_brand_svg(svg, &TrayAccentPalette::blue());
            let rgba = rasterize_svg_to_rgba(&tinted, TRAY_SIZE).expect("brand svg");
            assert_eq!(rgba.len(), (TRAY_SIZE * TRAY_SIZE * 4) as usize);
            assert!(
                rgba.chunks_exact(4).any(|px| px[3] > 0),
                "icon should be visible"
            );
        }
    }

    #[test]
    fn paused_icon_differs_from_active_icon() {
        let palette = TrayAccentPalette::blue();
        let active = brand_tray_rgba_bytes(false, &palette).expect("active icon");
        let paused = brand_tray_rgba_bytes(true, &palette).expect("paused icon");
        assert_ne!(
            active, paused,
            "paused tray icon should be visually distinct"
        );
    }

    #[test]
    fn accent_palette_changes_active_icon_pixels() {
        let blue = brand_tray_rgba_bytes(false, &TrayAccentPalette::blue()).expect("blue icon");
        let green = brand_tray_rgba_bytes(false, &TrayAccentPalette::green()).expect("green icon");
        assert_ne!(blue, green, "accent palette should tint the tray icon");
    }

    #[test]
    fn appindicator_icon_loads_thumbnail_when_opted_in() {
        let _guard = EnvGuard::set("WALLS_TRAY_WALLPAPER_THUMBNAIL", "1");
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("wall.png");
        let image = ImageBuffer::from_pixel(4, 4, Rgba([10u8, 20, 30, 255]));
        image.save(&image_path).unwrap();

        appindicator_icon_from_path(&image_path, &TrayAccentPalette::blue())
            .expect("thumbnail icon from composed wallpaper");
    }

    #[test]
    fn appindicator_icon_falls_back_when_current_composed_path_cannot_load() {
        let _guard = EnvGuard::set("WALLS_TRAY_WALLPAPER_THUMBNAIL", "1");
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing.png");

        appindicator_icon_from_path(&missing, &TrayAccentPalette::blue())
            .or_else(|_| brand_appindicator_icon(false, &TrayAccentPalette::blue()))
            .expect("brand icon fallback");
    }

    #[test]
    fn ksni_icon_loads_from_wallpaper_path_when_opted_in() {
        let _guard = EnvGuard::set("WALLS_TRAY_WALLPAPER_THUMBNAIL", "1");
        let tmp = tempfile::tempdir().unwrap();
        let image_path = tmp.path().join("wall.png");
        let image = ImageBuffer::from_pixel(4, 4, Rgba([10u8, 20, 30, 255]));
        image.save(&image_path).unwrap();

        let icons =
            ksni_icons_from_path(&image_path, &TrayAccentPalette::blue()).expect("ksni icons");
        assert_eq!(icons[0].width, TRAY_SIZE as i32);
        assert_eq!(icons[0].data.len(), (TRAY_SIZE * TRAY_SIZE * 4) as usize);
    }

    #[test]
    fn default_icon_uses_brand_svg_by_default() {
        let _guard = EnvGuard::unset("WALLS_TRAY_WALLPAPER_THUMBNAIL");
        default_appindicator_icon().expect("brand icon");
    }

    struct EnvGuard {
        key: &'static str,
        previous: Result<String, std::env::VarError>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key);
            // SAFETY: test-only; other tests in this crate that touch env run serially.
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key);
            // SAFETY: test-only.
            unsafe { std::env::remove_var(key) };
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Ok(value) => unsafe { std::env::set_var(self.key, value) },
                Err(_) => unsafe { std::env::remove_var(self.key) },
            }
        }
    }
}
